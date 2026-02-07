use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClobApiKeyCreds {
    pub key: String,
    pub secret: String,
    pub passphrase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GammaAuth {
    pub address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub topic: String,
    pub r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filters: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clob_auth: Option<ClobApiKeyCreds>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gamma_auth: Option<GammaAuth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionMessage {
    pub subscriptions: Vec<Subscription>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub topic: String,
    pub r#type: String,
    pub timestamp: u64,
    pub payload: serde_json::Value,
    pub connection_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
}

impl std::fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConnectionStatus::Connecting => write!(f, "CONNECTING"),
            ConnectionStatus::Connected => write!(f, "CONNECTED"),
            ConnectionStatus::Disconnected => write!(f, "DISCONNECTED"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SubscriptionAction {
    pub(crate) action: String,
    pub(crate) subscriptions: Vec<Subscription>,
}

impl From<SubscriptionMessage> for SubscriptionAction {
    fn from(msg: SubscriptionMessage) -> Self {
        Self {
            action: "subscribe".to_string(),
            subscriptions: msg.subscriptions,
        }
    }
}

impl SubscriptionMessage {
    pub fn to_subscribe_action(&self) -> SubscriptionAction {
        SubscriptionAction {
            action: "subscribe".to_string(),
            subscriptions: self.subscriptions.clone(),
        }
    }

    pub fn to_unsubscribe_action(&self) -> SubscriptionAction {
        SubscriptionAction {
            action: "unsubscribe".to_string(),
            subscriptions: self.subscriptions.clone(),
        }
    }
}
