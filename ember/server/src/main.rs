//! QUIC bridge to Feb17 inference.
//! Receives questions from smartphone via QUIC, forwards to Feb17 gRPC, returns AI answers.
//!
//! Usage: cargo run -p ember-server [-- [--inference http://127.0.0.1:50051]]
//! Requires: Feb17 grpc_server running (cargo run --bin grpc_server -p Feb17)

use std::net::SocketAddr;
use std::sync::Arc;

use chrono::Utc;
use quinn::{Endpoint, Incoming, ServerConfig};
use rcgen::{generate_simple_self_signed, CertifiedKey};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tonic::transport::Channel;
use tonic::Request;

mod de_kherud_grpc_llm {
    tonic::include_proto!("de.kherud.grpc.llm");
}

use de_kherud_grpc_llm::llm_client::LlmClient;
use de_kherud_grpc_llm::{ChatPrompt, CompleteRequest, InferenceParameters, Prompt};

fn log(prefix: &str, msg: &str) {
    let ts = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
    println!("[{ts}] {prefix} {msg}");
}

fn log_err(prefix: &str, msg: &str) {
    let ts = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
    eprintln!("[{ts}] {prefix} {msg}");
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    let mut inference_addr = "http://127.0.0.1:50051".to_string();
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--inference" && i + 1 < args.len() {
            inference_addr = args[i + 1].clone();
            i += 2;
            continue;
        }
        i += 1;
    }

    let listen_addr: SocketAddr = "0.0.0.0:4433".parse()?;
    run(listen_addr, inference_addr)
}

#[tokio::main]
async fn run(
    listen_addr: SocketAddr,
    inference_addr: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server_config = configure_server()?;
    let endpoint = Endpoint::server(server_config, listen_addr)?;

    log("SERVER", &format!("listening on {}", endpoint.local_addr()?));
    log("SERVER", &format!("inference: {} (connects on first request)", inference_addr));
    log("SERVER", "monitoring all app↔server traffic");

    // Lazy gRPC client - connects on first request so server starts even if Feb17 isn't ready
    let grpc_client: Arc<tokio::sync::Mutex<Option<LlmClient<Channel>>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    let inference_addr = Arc::new(inference_addr);

    while let Some(incoming) = endpoint.accept().await {
        let client_opt = grpc_client.clone();
        let addr = inference_addr.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(incoming, client_opt, addr).await {
                log_err("CONN", &format!("error: {e}"));
            }
        });
    }

    Ok(())
}

fn configure_server() -> Result<ServerConfig, Box<dyn std::error::Error + Send + Sync>> {
    let CertifiedKey { cert, key_pair } =
        generate_simple_self_signed(vec!["localhost".to_string(), "127.0.0.1".to_string()])?;
    let cert_der = CertificateDer::from(cert);
    let key = PrivatePkcs8KeyDer::from(key_pair.serialize_der());
    let key_der = PrivateKeyDer::try_from(key)?;

    let server_config = ServerConfig::with_single_cert(vec![cert_der], key_der)?;
    Ok(server_config)
}

async fn handle_connection(
    incoming: Incoming,
    grpc_client: Arc<tokio::sync::Mutex<Option<LlmClient<Channel>>>>,
    inference_addr: Arc<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let connection = incoming.await?;
    let remote = connection.remote_address();
    log("CONN", &format!("client connected from {}", remote));

    loop {
        let (mut send, mut recv) = match connection.accept_bi().await {
            Ok(stream) => stream,
            Err(quinn::ConnectionError::ApplicationClosed(_)) => break,
            Err(e) => return Err(e.into()),
        };

        let client_opt = grpc_client.clone();
        let addr = inference_addr.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_stream(&mut send, &mut recv, client_opt, addr).await {
                log_err("STREAM", &format!("error: {e}"));
            }
        });
    }

    Ok(())
}

async fn handle_stream(
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
    grpc_client: Arc<tokio::sync::Mutex<Option<LlmClient<Channel>>>>,
    inference_addr: Arc<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let prompt_bytes = recv.read_to_end(64 * 1024).await?;
    let prompt = String::from_utf8_lossy(&prompt_bytes).trim().to_string();

    log("RECV", &format!("{} bytes from app", prompt_bytes.len()));
    log("RECV", &format!("prompt ({} chars): {}", prompt.len(), prompt));

    if prompt.is_empty() {
        let err = b"Error: empty prompt";
        log("SEND", "Error: empty prompt");
        send.write_all(err).await?;
        send.finish()?;
        return Ok(());
    }

    let response = match call_inference(&grpc_client, &inference_addr, &prompt).await {
        Ok(r) => r,
        Err(e) => {
            let err = format!("Error: {}", e);
            log_err("SEND", &err);
            send.write_all(err.as_bytes()).await?;
            send.finish()?;
            return Ok(());
        }
    };

    log("SEND", &format!("response ({} chars): {}", response.len(), response));
    send.write_all(response.as_bytes()).await?;
    send.finish()?;
    Ok(())
}

async fn call_inference(
    client_opt: &Arc<tokio::sync::Mutex<Option<LlmClient<Channel>>>>,
    inference_addr: &str,
    prompt: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut guard = client_opt.lock().await;
    if guard.is_none() {
        let channel = Channel::from_shared(inference_addr.to_string())?
            .connect()
            .await
            .map_err(|e| format!("gRPC connect failed: {e}"))?;
        *guard = Some(LlmClient::new(channel));
    }
    let client = guard.as_mut().unwrap();

    let complete_req = CompleteRequest {
        prompt: Some(Prompt {
            prompt: Some(de_kherud_grpc_llm::prompt::Prompt::Chat(ChatPrompt {
                prompt: prompt.to_string(),
            })),
        }),
        parameters: Some(InferenceParameters {
            n_predict: Some(256),
            temp: Some(0.7),
            top_k: Some(40),
            top_p: Some(0.9),
            penalty_last_n: Some(64),
            penalty_repeat: Some(1.1),
            ..Default::default()
        }),
    };

    let reply = client
        .complete(Request::new(complete_req))
        .await
        .map_err(|e| format!("inference failed: {e}"))?;

    Ok(reply.into_inner().response)
}
