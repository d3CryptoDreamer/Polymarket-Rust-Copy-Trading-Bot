use crate::real_time_data_client::model::{
    ConnectionStatus, Message, SubscriptionMessage,
};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio::time::interval;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

const DEFAULT_HOST: &str = "wss://ws-live-data.polymarket.com";
const DEFAULT_PING_INTERVAL: u64 = 5000;

pub type OnConnectCallback = dyn Fn() + Send + Sync;
pub type OnMessageCallback = dyn Fn(Message) + Send + Sync;
pub type OnStatusChangeCallback = dyn Fn(ConnectionStatus) + Send + Sync;

pub struct RealTimeDataClientArgs {
    pub on_connect: Option<Box<OnConnectCallback>>,
    pub on_message: Option<Box<OnMessageCallback>>,
    pub on_status_change: Option<Box<OnStatusChangeCallback>>,
    pub host: Option<String>,
    pub ping_interval: Option<u64>,
    pub auto_reconnect: Option<bool>,
}

impl Default for RealTimeDataClientArgs {
    fn default() -> Self {
        Self {
            on_connect: None,
            on_message: None,
            on_status_change: None,
            host: None,
            ping_interval: None,
            auto_reconnect: Some(true),
        }
    }
}

pub struct RealTimeDataClient {
    host: String,
    ping_interval: Duration,
    auto_reconnect: Arc<RwLock<bool>>,
    on_connect: Option<Arc<OnConnectCallback>>,
    on_message: Option<Arc<OnMessageCallback>>,
    on_status_change: Option<Arc<OnStatusChangeCallback>>,
    command_tx: Option<mpsc::UnboundedSender<ClientCommand>>,
}

enum ClientCommand {
    Connect,
    Disconnect,
    Subscribe(SubscriptionMessage),
    Unsubscribe(SubscriptionMessage),
}

impl RealTimeDataClient {
    pub fn new(args: RealTimeDataClientArgs) -> Self {
        let host = args.host.unwrap_or_else(|| DEFAULT_HOST.to_string());
        let ping_interval_ms = args.ping_interval.unwrap_or(DEFAULT_PING_INTERVAL);
        let ping_interval = Duration::from_millis(ping_interval_ms);
        let auto_reconnect = Arc::new(RwLock::new(args.auto_reconnect.unwrap_or(true)));

        Self {
            host,
            ping_interval,
            auto_reconnect,
            on_connect: args.on_connect.map(|cb| Arc::from(cb)),
            on_message: args.on_message.map(|cb| Arc::from(cb)),
            on_status_change: args.on_status_change.map(|cb| Arc::from(cb)),
            command_tx: None,
        }
    }

    pub async fn connect(&mut self) -> Result<(), String> {
        self.notify_status_change(ConnectionStatus::Connecting);

        let (command_tx, command_rx) = mpsc::unbounded_channel();
        self.command_tx = Some(command_tx.clone());

        let host = self.host.clone();
        let ping_interval = self.ping_interval;
        let auto_reconnect = Arc::clone(&self.auto_reconnect);
        let on_connect = self.on_connect.clone();
        let on_message = self.on_message.clone();
        let on_status_change = self.on_status_change.clone();

        tokio::spawn(async move {
            Self::connection_task(
                host,
                ping_interval,
                auto_reconnect,
                command_rx,
                on_connect,
                on_message,
                on_status_change,
            )
            .await;
        });

        Ok(())
    }

    async fn connection_task(
        host: String,
        ping_interval: Duration,
        auto_reconnect: Arc<RwLock<bool>>,
        mut command_rx: mpsc::UnboundedReceiver<ClientCommand>,
        on_connect: Option<Arc<OnConnectCallback>>,
        on_message: Option<Arc<OnMessageCallback>>,
        on_status_change: Option<Arc<OnStatusChangeCallback>>,
    ) {
        loop {
            let should_reconnect = *auto_reconnect.read().await;
            if !should_reconnect {
                break;
            }

            match connect_async(&host).await {
                Ok((ws_stream, _)) => {
                    if let Some(cb) = &on_status_change {
                        cb(ConnectionStatus::Connected);
                    }

                    if let Some(cb) = &on_connect {
                        cb();
                    }

                    let (mut write, mut read) = ws_stream.split();
                    let mut ping_interval = interval(ping_interval);
                    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

                    let mut _connection_closed = false;

                    loop {
                        tokio::select! {
                            msg = read.next() => {
                                match msg {
                                    Some(Ok(WsMessage::Text(text))) => {
                                        if text == "pong" {
                                            continue;
                                        }

                                        if text.contains("payload") {
                                            if let Ok(message) = serde_json::from_str::<Message>(&text) {
                                                if let Some(cb) = &on_message {
                                                    cb(message);
                                                }
                                            }
                                        }
                                    }
                                    Some(Ok(WsMessage::Close(_))) => {
                                        _connection_closed = true;
                                        break;
                                    }
                                    Some(Err(e)) => {
                                        eprintln!("WebSocket error: {}", e);
                                        _connection_closed = true;
                                        break;
                                    }
                                    None => {
                                        _connection_closed = true;
                                        break;
                                    }
                                    _ => {}
                                }
                            }

                            _ = ping_interval.tick() => {
                                if let Err(e) = write.send(WsMessage::Text("ping".to_string())).await {
                                    eprintln!("Error sending ping: {}", e);
                                    _connection_closed = true;
                                    break;
                                }
                            }

                            cmd = command_rx.recv() => {
                                match cmd {
                                    Some(ClientCommand::Disconnect) => {
                                        let _ = write.close().await;
                                        _connection_closed = true;
                                        break;
                                    }
                                    Some(ClientCommand::Subscribe(msg)) => {
                                        let action = msg.to_subscribe_action();
                                        if let Ok(json) = serde_json::to_string(&action) {
                                            if let Err(e) = write.send(WsMessage::Text(json)).await {
                                                eprintln!("Error sending subscribe: {}", e);
                                            }
                                        }
                                    }
                                    Some(ClientCommand::Unsubscribe(msg)) => {
                                        let action = msg.to_unsubscribe_action();
                                        if let Ok(json) = serde_json::to_string(&action) {
                                            if let Err(e) = write.send(WsMessage::Text(json)).await {
                                                eprintln!("Error sending unsubscribe: {}", e);
                                            }
                                        }
                                    }
                                    Some(ClientCommand::Connect) => {}
                                    None => {
                                        _connection_closed = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    if let Some(cb) = &on_status_change {
                        cb(ConnectionStatus::Disconnected);
                    }

                    if _connection_closed && *auto_reconnect.read().await {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    } else {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Connection error: {}", e);
                    if let Some(cb) = &on_status_change {
                        cb(ConnectionStatus::Disconnected);
                    }

                    if *auto_reconnect.read().await {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    } else {
                        break;
                    }
                }
            }
        }
    }

    pub async fn disconnect(&self) {
        *self.auto_reconnect.write().await = false;
        if let Some(tx) = &self.command_tx {
            let _ = tx.send(ClientCommand::Disconnect);
        }
    }

    pub fn subscribe(&self, msg: SubscriptionMessage) {
        if let Some(tx) = &self.command_tx {
            let _ = tx.send(ClientCommand::Subscribe(msg));
        }
    }

    pub fn unsubscribe(&self, msg: SubscriptionMessage) {
        if let Some(tx) = &self.command_tx {
            let _ = tx.send(ClientCommand::Unsubscribe(msg));
        }
    }

    fn notify_status_change(&self, status: ConnectionStatus) {
        if let Some(cb) = &self.on_status_change {
            cb(status);
        }
    }
}

impl Default for RealTimeDataClient {
    fn default() -> Self {
        Self::new(RealTimeDataClientArgs::default())
    }
}
