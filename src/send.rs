use anyhow::Result;
use lapin::{self};

use std::result::Result::Ok;
use tokio;

pub fn send_log_local(client_name: String, ack_text: String) {
    tokio::spawn(async move {
        if let Err(err) = send_log_local_impl(client_name.as_str(), ack_text.as_str()).await {
            eprintln!("Error in send: {}", err);
        }
    });
}

async fn send_log_local_impl(client_name: &str, ack_text: &str) -> Result<()> {
    let addr = "amqp://127.0.0.1:5672/%2f";
    let conn = lapin::Connection::connect(addr, lapin::ConnectionProperties::default()).await?;
    let channel = conn.create_channel().await?;
    let exchange_name: &str = "logging";
    let _ = channel
        .exchange_declare(
            exchange_name,
            lapin::ExchangeKind::Topic,
            lapin::options::ExchangeDeclareOptions::default(),
            lapin::types::FieldTable::default(),
        )
        .await?;
    channel
        .basic_publish(
            exchange_name,
            format!("logging.rpc.{}", client_name).as_str(),
            lapin::options::BasicPublishOptions::default(),
            ack_text.as_bytes(),
            lapin::BasicProperties::default(),
        )
        .await?;
    Ok(())
}
