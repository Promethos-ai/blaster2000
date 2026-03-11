//! QUIC bridge to Feb17 inference.
//! Receives questions from smartphone via QUIC, forwards to Feb17 gRPC, returns AI answers.
//!
//! Usage: cargo run -p ember-server [-- [--inference http://127.0.0.1:50051] [--log-file PATH] [--no-log-file]]
//! Requires: Feb17 grpc_server running (cargo run --bin grpc_server -p Feb17)

use std::fs::OpenOptions;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;

use chrono::Utc;
use quinn::{Endpoint, Incoming, ServerConfig};
use rcgen::{generate_simple_self_signed, CertifiedKey};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde_json::json;
use tokio_stream::StreamExt;
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

/// Writes a line to the connection log file (if configured). Format: timestamp\tremote\ttype\tdetails
fn log_connection(log: &Option<Arc<std::sync::Mutex<std::fs::File>>>, remote: &SocketAddr, event: &str, details: &str) {
    if let Some(file) = log {
        let ts = Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let line = format!("{ts}\t{remote}\t{event}\t{details}\n");
        if let Ok(mut f) = file.lock() {
            let _ = f.write_all(line.as_bytes());
            let _ = f.flush();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    let mut inference_addr = "http://127.0.0.1:50051".to_string();
    let mut log_file: Option<String> = Some("ember-connections.log".to_string());
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--inference" && i + 1 < args.len() {
            inference_addr = args[i + 1].clone();
            i += 2;
            continue;
        }
        if args[i] == "--log-file" && i + 1 < args.len() {
            log_file = Some(args[i + 1].clone());
            i += 2;
            continue;
        }
        if args[i] == "--no-log-file" {
            log_file = None;
            i += 1;
            continue;
        }
        i += 1;
    }

    let conn_log: Option<Arc<std::sync::Mutex<std::fs::File>>> = log_file.as_ref().and_then(|path| {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map(|f| Arc::new(std::sync::Mutex::new(f)))
            .ok()
    });
    if let Some(ref path) = log_file {
        if conn_log.is_some() {
            log("SERVER", &format!("connection log: {}", path));
        } else {
            log_err("SERVER", &format!("failed to open log file: {}", path));
        }
    }

    let listen_addr: SocketAddr = "0.0.0.0:4433".parse()?;
    run(listen_addr, inference_addr, conn_log)
}

#[tokio::main]
async fn run(
    listen_addr: SocketAddr,
    inference_addr: String,
    conn_log: Option<Arc<std::sync::Mutex<std::fs::File>>>,
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
        let log = conn_log.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(incoming, client_opt, addr, log).await {
                log_err("CONN", &format!("error: {e}"));
            }
        });
    }

    Ok(())
}

fn configure_server() -> Result<ServerConfig, Box<dyn std::error::Error + Send + Sync>> {
    let names = vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
        "192.168.1.27".to_string(),
        "192.168.1.238".to_string(),
        "172.16.0.1".to_string(),
    ];
    let CertifiedKey { cert, key_pair } = generate_simple_self_signed(names)?;
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
    conn_log: Option<Arc<std::sync::Mutex<std::fs::File>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let connection = incoming.await?;
    let remote = connection.remote_address();
    log("CONN", &format!("client connected from {}", remote));
    log_connection(&conn_log, &remote, "CONNECT", "client connected");

    loop {
        let (mut send, mut recv) = match connection.accept_bi().await {
            Ok(stream) => stream,
            Err(quinn::ConnectionError::ApplicationClosed(_)) => break,
            Err(e) => return Err(e.into()),
        };

        let client_opt = grpc_client.clone();
        let addr = inference_addr.clone();
        let log = conn_log.clone();
        let remote_addr = remote;
        tokio::spawn(async move {
            if let Err(e) = handle_stream(&mut send, &mut recv, client_opt, addr, &log, remote_addr).await {
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
    conn_log: &Option<Arc<std::sync::Mutex<std::fs::File>>>,
    remote: SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let prompt_bytes = recv.read_to_end(64 * 1024).await?;
    let prompt = String::from_utf8_lossy(&prompt_bytes).trim().to_string();

    log("RECV", &format!("{} bytes from app", prompt_bytes.len()));
    log("RECV", &format!("prompt ({} chars): {}", prompt.len(), prompt));

    // Truncate prompt for log file (first 200 chars)
    let prompt_preview = if prompt.len() > 200 {
        format!("{}...", &prompt[..200])
    } else {
        prompt.clone()
    };
    let prompt_preview_escaped = prompt_preview.replace('\t', " ").replace('\n', " ");
    log_connection(conn_log, &remote, "REQUEST", &format!("prompt_len={} prompt={}", prompt.len(), prompt_preview_escaped));

    if prompt.is_empty() {
        send_stream_error(send, 0, "empty prompt").await?;
        log_connection(conn_log, &remote, "ERROR", "empty prompt");
        return Ok(());
    }

    let interaction_id = {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    };

    if let Err(e) = stream_inference(send, &grpc_client, &inference_addr, &prompt, interaction_id, conn_log, &remote).await {
        log_err("SEND", &format!("{e}"));
        log_connection(conn_log, &remote, "ERROR", &e.to_string().replace('\t', " ").replace('\n', " "));
        send_stream_error(send, interaction_id, &e.to_string()).await?;
    }
    Ok(())
}

fn stream_frame(obj: serde_json::Value) -> Vec<u8> {
    let mut v = serde_json::to_vec(&obj).unwrap();
    v.push(b'\n');
    v
}

async fn send_stream_error(send: &mut quinn::SendStream, interaction_id: u64, err: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let frame = stream_frame(json!({
        "type": "stream_error",
        "error": err,
        "interaction_id": interaction_id
    }));
    send.write_all(&frame).await?;
    send.finish()?;
    Ok(())
}

async fn stream_inference(
    send: &mut quinn::SendStream,
    grpc_client: &Arc<tokio::sync::Mutex<Option<LlmClient<Channel>>>>,
    inference_addr: &Arc<String>,
    prompt: &str,
    interaction_id: u64,
    conn_log: &Option<Arc<std::sync::Mutex<std::fs::File>>>,
    remote: &SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // stream_start
    let frame = stream_frame(json!({
        "type": "stream_start",
        "interaction_id": interaction_id
    }));
    send.write_all(&frame).await?;

    let mut total_len = 0usize;
    let mut stream = call_inference_stream(grpc_client, inference_addr, prompt).await?;

    while let Some(result) = stream.next().await {
        match result {
            Ok(reply) => {
                let token = reply.token;
                if !token.is_empty() {
                    total_len += token.len();
                    let frame = stream_frame(json!({
                        "type": "stream_token",
                        "token": token,
                        "interaction_id": interaction_id
                    }));
                    send.write_all(&frame).await?;
                }
            }
            Err(e) => return Err(format!("inference stream error: {e}").into()),
        }
    }

    // stream_end
    let frame = stream_frame(json!({
        "type": "stream_end",
        "interaction_id": interaction_id
    }));
    send.write_all(&frame).await?;
    send.finish()?;

    log("SEND", &format!("response stream ({} chars)", total_len));
    log_connection(conn_log, remote, "RESPONSE", &format!("response_len={}", total_len));
    Ok(())
}

async fn call_inference_stream(
    client_opt: &Arc<tokio::sync::Mutex<Option<LlmClient<Channel>>>>,
    inference_addr: &str,
    prompt: &str,
) -> Result<tonic::Streaming<de_kherud_grpc_llm::CompleteStreamReply>, Box<dyn std::error::Error + Send + Sync>> {
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

    let stream = client
        .complete_stream(Request::new(complete_req))
        .await
        .map_err(|e| format!("inference stream failed: {e}"))?
        .into_inner();

    Ok(stream)
}
