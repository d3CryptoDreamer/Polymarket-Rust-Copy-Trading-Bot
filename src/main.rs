mod real_time_data_client;
mod util;


use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use std::str::FromStr as _;
use std::env;
use alloy::primitives::U256;
use alloy::providers::ProviderBuilder;
use alloy::signers::Signer as _;
use alloy::signers::local::LocalSigner;
use polymarket_client_sdk::clob::{Client, Config};
use polymarket_client_sdk::types::{Amount, AssetType, BalanceAllowanceRequestBuilder, OrderType, Side};
use polymarket_client_sdk::{POLYGON, PRIVATE_KEY_VAR, contract_config};


use crate::util::set_all_approvals;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::Utc;
use std::fs::OpenOptions;
use std::io::Write;
use crate::real_time_data_client::{
    ConnectionStatus, Message, RealTimeDataClient, RealTimeDataClientArgs, Subscription,
    SubscriptionMessage,
};

#[derive(Serialize, Deserialize)]
struct CredentialsJson {
    #[serde(alias = "apiKey")]
    key: Uuid,
    secret: String,
    passphrase: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenv::dotenv().ok();
    let enable_trading = false;
    let private_key = std::env::var(PRIVATE_KEY_VAR).expect("Need a private key");
    let signer = LocalSigner::from_str(&private_key)?.with_chain_id(Some(POLYGON));
    let client = Client::new("https://clob.polymarket.com", Config::default())?
        .authentication_builder(&signer)
        .authenticate()
        .await?;

    let multiplier = env::var("MULTIPLIER").unwrap_or("1".to_string()).parse::<f64>().unwrap();
    let min_value = Decimal::from_f64(1.0).unwrap();
    let max_value = Decimal::from_f64(4.0).unwrap();
    const RPC_URL: &str = "https://polygon-rpc.com";
    
    let provider = ProviderBuilder::new()
        .wallet(signer.clone())
        .connect(RPC_URL)
        .await?;

    let config = contract_config(POLYGON, false).unwrap();
    let neg_risk_config = contract_config(POLYGON, true).unwrap();
    
    let allowance_amount = U256::MAX;
    
    println!("Setting all approvals for Polymarket trading...");
    set_all_approvals(&provider, signer.address(), &config, &neg_risk_config, allowance_amount).await?;

    let on_connect = Box::new(|| {
        println!("Connected to WebSocket server");
    });

    let on_status_change = Box::new(|status: ConnectionStatus| {
        println!("Connection status changed: {}", status);
    });
    const TARGET_WALLET: &str = "afewfdzgre";

    let client_clone = client.clone();
    let signer_clone = signer.clone();
    
    let on_message = Box::new(move |message: Message| {
        if message.payload.get("name").and_then(|v| v.as_str()) == Some(TARGET_WALLET) {
            println!("message: {:?}", message.payload.get("proxyWallet"));
        }
        let mut log_file = OpenOptions::new()
    .create(true)
    .append(true)
    .open("log.txt")
    .unwrap();

    log_file
        .write_all(
            message
                .payload
                .unwrap()
                .to_string()
                .as_bytes(),
        )
        .unwrap();

    log_file.write_all(b"\n").unwrap();
        if !enable_trading {
            return;
        }
        let Some(proxy_wallet) = message.payload.get("name")
            .and_then(|v| v.as_str()) else { return; };

        if proxy_wallet != TARGET_WALLET {
            return;
        }
        let Some(side) = message.payload.get("side").and_then(|v| v.as_str()) else { return; };
        if side != "BUY" {
            return;
        }
        let Some(size) = message.payload.get("size").and_then(|v| v.as_f64()) else { return; };

        let client = client_clone.clone();
        let signer = signer_clone.clone();
        
        
            tokio::spawn(async move {
                let message_payload = message.payload.clone();
                let token_id = match message_payload.get("asset").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return,
                };
                let Some(price) = message_payload.get("price").and_then(|v| v.as_f64()) else { return; };
                let price_decimal = Decimal::from_f64(price).unwrap();
                let size_decimal = Decimal::from_f64(size).unwrap();
                let multiplier_decimal = Decimal::from_f64(multiplier).unwrap();
                let mut buy_amount_value = price_decimal * size_decimal * multiplier_decimal;
                
                if buy_amount_value < min_value {
                    buy_amount_value = min_value;
                }
                if buy_amount_value > max_value {
                    buy_amount_value = max_value;
                }           
                
                let buy_amount = Amount::usdc(buy_amount_value).unwrap();
                let market_order = match client
                    .market_order()
                    .token_id(token_id)
                    .order_type(OrderType::FAK)
                    .amount(buy_amount)
                    .side(Side::Buy)
                    .build()
                    .await
                {
                    Ok(order) => order,
                    Err(e) => {
                        eprintln!("Error building market order: {:?}", e);
                        return;
                    }
                };

                let signed_order = match client.sign(&signer, market_order).await {
                    Ok(order) => order,
                    Err(e) => {
                        eprintln!("Error signing order: {:?}", e);
                        return;
                    }
                };

                match client.post_order(signed_order).await {
                    Ok(response) => {
                        println!("Order submitted successfully for token_id {}: {:?}", token_id, response);
                    }
                    Err(e) => {
                        eprintln!("Error submitting order for token_id {}: {:?}", token_id, e);
                    }
                }
            });
        
    });
    let mut client = RealTimeDataClient::new(RealTimeDataClientArgs {
        on_connect: Some(on_connect),
        on_message: Some(on_message),
        on_status_change: Some(on_status_change),
        host: None,
        ping_interval: None,
        auto_reconnect: Some(true),
    });

    client.connect().await.map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, e)) as Box<dyn std::error::Error + Send + Sync>)?;

    client.subscribe(SubscriptionMessage {
        subscriptions: vec![
            Subscription {
                topic: "activity".to_string(),
                r#type: "trades".to_string(),
                filters: None,
                clob_auth: None,
                gamma_auth: None,
            },
        ],
    });

    tokio::signal::ctrl_c().await?;
    println!("\nShutting down...");
    client.disconnect().await;

    Ok(())
}
