use anyhow::{Result, anyhow};
use clap::Parser;
use derive_more::Debug;
use futures_util::stream::StreamExt;
use lapin::{
    Connection, ConnectionProperties, ExchangeKind,
    options::{
        BasicAckOptions, BasicConsumeOptions, ExchangeDeclareOptions, QueueBindOptions,
        QueueDeclareOptions,
    },
    tcp::{OwnedIdentity, OwnedTLSConfig},
    types::FieldTable,
};
use serde::Deserialize;
use std::fs;
use std::result::Result::Ok;

mod git_cred;
mod send;
mod shell;

struct RabbitMQCerts {
    cafile: String,
    certfile: String,
    keyfile: String,
    use_crt: bool,
}

#[derive(Deserialize, Debug)]
struct ConnectionConfig {
    host: String,
    user: String,
    #[debug(ignore)]
    pass: String,
    vhost: Option<String>,
    client_name: Option<String>,
    port: Option<u16>,
    is_secure: bool,
    cafile: Option<String>,
    certfile: Option<String>,
    keyfile: Option<String>,
}

struct AnalyzedParams {
    amqp_addr: String,
    amqp_addr_for_print: String,
    routing_key: String,
    client_name: String,
    cafile: Option<String>,
    certfile: Option<String>,
    keyfile: Option<String>,
}

#[derive(Deserialize, Debug)]
struct DeliveryData {
    sender: String,
    method: String,
    payload: Option<String>,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    config: String,
}

fn get_tls_config(cafile: &str, certfile: &str, keyfile: &str) -> Result<OwnedTLSConfig> {
    let cert_chain = fs::read_to_string(cafile)?;
    let client_cert = fs::read(certfile)?;
    let client_key = fs::read(keyfile)?;

    Ok(OwnedTLSConfig {
        identity: Some(OwnedIdentity::PKCS8 {
            pem: client_cert,
            key: client_key,
        }),
        cert_chain: Some(cert_chain),
    })
}

fn test_receive(client_name: String) {
    println!("[test] Test message received");
    send::send_log_local(client_name, "Test message received".into());
}

async fn dispatcher(delivery_data: DeliveryData, client_name: &String) {
    let sender = delivery_data.sender;
    if sender == *client_name {
        return;
    }

    let method = delivery_data.method;

    match method.as_str() {
        "git-credential" => {
            if let Some(payload) = delivery_data.payload
                && let Some(cred) = serde_json::from_str::<git_cred::GitCredential>(&payload).ok()
            {
                let _ = git_cred::process_credential(client_name.clone(), cred).await;
            } else {
                eprintln!("Invalid git credential JSON");
                send::send_log_local(
                    client_name.clone(),
                    "[git-credential] Invalid git credential JSON".into(),
                );
            }
        }
        "shell" => {
            if let Some(payload) = delivery_data.payload
                && let Some(shell_param) = serde_json::from_str::<shell::ShellParam>(&payload).ok()
            {
                shell::process_shell(client_name.clone(), shell_param);
            } else {
                eprintln!("Invalid shell JSON");
                send::send_log_local(client_name.clone(), "[shell] Invalid shell JSON".into());
            }
        }
        "test" => {
            test_receive(client_name.clone());
        }
        _ => {
            eprintln!("Unknown method: {}", method);
        }
    }
}

async fn on_delivery(mut delivery: lapin::message::Delivery, client_name: &String) -> Result<()> {
    let data = std::mem::take(&mut delivery.data);
    tokio::spawn(async move {
        let _ = delivery.ack(BasicAckOptions::default()).await;
    });
    let body_str = String::from_utf8_lossy(&data);
    println!("Received message: {}", body_str);
    match serde_json::from_str::<DeliveryData>(&body_str) {
        Ok(delivery_data) => {
            dispatcher(delivery_data, client_name).await;
        }
        Err(e) => {
            eprintln!("Invalid JSON: {}", e);
        }
    }

    Ok(())
}

async fn connection_loop(
    amqp_addr: &String,
    amqp_addr_for_print: &String,
    auth_cert: &RabbitMQCerts,
    routing_key: &String,
    client_name: &String,
) -> Result<()> {
    let conn: Connection;
    match auth_cert.use_crt {
        true => {
            conn = Connection::connect_with_config(
                &amqp_addr.as_str(),
                ConnectionProperties::default(),
                get_tls_config(&auth_cert.cafile, &auth_cert.certfile, &auth_cert.keyfile)?,
            )
            .await?;
        }
        false => {
            conn = Connection::connect(&amqp_addr, ConnectionProperties::default()).await?;
        }
    }
    // channel, queue declare
    let channel = conn.create_channel().await?;
    let queue = channel
        .queue_declare(
            "",
            QueueDeclareOptions {
                exclusive: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    // exchanges
    let exchange = routing_key.split(".").next().unwrap_or("unknown");
    channel
        .exchange_declare(
            &exchange,
            ExchangeKind::Topic,
            ExchangeDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;
    channel
        .exchange_declare(
            "all",
            ExchangeKind::Topic,
            ExchangeDeclareOptions::default(),
            FieldTable::default(),
        )
        .await?;

    // queue bind
    channel
        .queue_bind(
            queue.name().as_str(),
            &exchange,
            &routing_key,
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;
    channel
        .queue_bind(
            queue.name().as_str(),
            "all",
            "all.#",
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;

    println!(
        "Connected to RabbitMQ at {}, client exchange = {}, client routing_key = {}",
        amqp_addr_for_print, exchange, routing_key
    );

    let mut consumer = channel
        .basic_consume(
            queue.name().as_str(),
            "my_consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    while let Some(delivery) = consumer.next().await {
        match delivery {
            Ok(delivery) => {
                let _ = on_delivery(delivery, client_name).await;
            }
            Err(e) => {
                return Err(anyhow!("Failed to consume message: {:?}", e));
            }
        }
    }
    Ok(())
}

fn analyze_amqp_addr(args: &Args) -> AnalyzedParams {
    let config_json_str = fs::read_to_string(&args.config).expect("Failed to read config file");
    let args: ConnectionConfig;
    match serde_json::from_str::<ConnectionConfig>(&config_json_str) {
        Ok(c) => args = c,
        Err(e) => panic!("Failed to parse config file: {}", e),
    }
    let mut amqp_addr = format!(
        "{}://{}:{}@{}:{}",
        if args.is_secure { "amqps" } else { "amqp" },
        args.user,
        args.pass,
        args.host,
        args.port
            .unwrap_or_else(|| args.is_secure.then_some(5671).unwrap_or(5672))
    );
    let mut amqp_addr_for_print = format!(
        "{}://{}:{}@{}:{}",
        if args.is_secure { "amqps" } else { "amqp" },
        args.user,
        "***".to_owned(),
        args.host,
        args.port
            .unwrap_or_else(|| args.is_secure.then_some(5671).unwrap_or(5672))
    );
    if let Some(vhost) = &args.vhost {
        amqp_addr = format!("{}/{}", amqp_addr, vhost);
        amqp_addr_for_print = format!("{}/{}", amqp_addr_for_print, vhost);
    }
    let client_name = args.client_name.unwrap_or_else(|| args.user.clone());
    if client_name == "all" {
        panic!("client_name cannot be 'all'");
    }
    let routing_key = format!("{}.#", client_name);
    AnalyzedParams {
        amqp_addr,
        amqp_addr_for_print,
        routing_key,
        client_name,
        cafile: args.cafile,
        certfile: args.certfile,
        keyfile: args.keyfile,
    }
}

fn rabbit_robust_connect(
    amqp_addr: String,
    amqp_addr_for_print: String,
    auth_cert: RabbitMQCerts,
    routing_key: String,
    client_name: String,
) {
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
        eprintln!("Connecting to rabbitmq");
        try_connection_loop(
            amqp_addr,
            amqp_addr_for_print,
            auth_cert,
            routing_key,
            client_name,
        )
        .await;
    });
}

async fn try_connection_loop(
    amqp_addr: String,
    amqp_addr_for_print: String,
    auth_cert: RabbitMQCerts,
    routing_key: String,
    client_name: String,
) {
    if let Err(err) = connection_loop(
        &amqp_addr,
        &amqp_addr_for_print,
        &auth_cert,
        &routing_key,
        &client_name,
    )
    .await
    {
        eprintln!("Error: {}", err);
        rabbit_robust_connect(
            amqp_addr,
            amqp_addr_for_print,
            auth_cert,
            routing_key,
            client_name,
        );
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let params = analyze_amqp_addr(&args);

    // let conn: Connection;
    let auth_cert: RabbitMQCerts;

    if params.cafile.is_some() || params.certfile.is_some() || params.keyfile.is_some() {
        // all of these must be set
        if params.cafile.is_none() || params.certfile.is_none() || params.keyfile.is_none() {
            return Err(anyhow!("cafile, certfile and keyfile must be all set"));
        }
        let cafile = params.cafile.unwrap_or_else(|| "".into());
        let certfile = params.certfile.unwrap_or_else(|| "".into());
        let keyfile = params.keyfile.unwrap_or_else(|| "".into());
        auth_cert = RabbitMQCerts {
            cafile,
            certfile,
            keyfile,
            use_crt: true,
        };
    } else {
        auth_cert = RabbitMQCerts {
            cafile: "".into(),
            certfile: "".into(),
            keyfile: "".into(),
            use_crt: false,
        };
    }

    rabbit_robust_connect(
        params.amqp_addr,
        params.amqp_addr_for_print,
        auth_cert,
        params.routing_key,
        params.client_name,
    );
    std::future::pending::<()>().await;
    Ok(())
}
