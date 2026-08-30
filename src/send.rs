use anyhow::Result;
use lapin::{self};

use std::result::Result::Ok;
use tokio;

const EXCHANGE_NAME: &str = "logging";

pub fn send_log_local(client_name: String, ack_text: String) {
    let routing_key = format!("logging.rpc.{}", client_name);
    tokio::spawn(async move {
        if let Err(err) = send_impl(&routing_key, ack_text.as_bytes()).await {
            eprintln!("Error in send: {}", err);
        }
    });
}

/// Publish the full result of a shell call back to the caller, keyed by
/// `logging.shell.{client_name}` so callers can subscribe just for outputs.
pub fn send_shell_result(client_name: String, payload: String) {
    let routing_key = format!("logging.shell.{}", client_name);
    tokio::spawn(async move {
        if let Err(err) = send_impl(&routing_key, payload.as_bytes()).await {
            eprintln!("Error in send shell result: {}", err);
        }
    });
}

async fn send_impl(routing_key: &str, body: &[u8]) -> Result<()> {
    let addr = "amqp://127.0.0.1:5672/%2f";
    let conn = lapin::Connection::connect(addr, lapin::ConnectionProperties::default()).await?;
    let channel = conn.create_channel().await?;
    let _ = channel
        .exchange_declare(
            EXCHANGE_NAME,
            lapin::ExchangeKind::Topic,
            lapin::options::ExchangeDeclareOptions::default(),
            lapin::types::FieldTable::default(),
        )
        .await?;
    channel
        .basic_publish(
            EXCHANGE_NAME,
            routing_key,
            lapin::options::BasicPublishOptions::default(),
            body,
            lapin::BasicProperties::default(),
        )
        .await?;
    Ok(())
}
