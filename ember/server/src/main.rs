//! QUIC bridge to Feb17 inference.
//! Receives questions from smartphone via QUIC, forwards to Feb17 gRPC over QUIC, returns AI answers.
//!
//! Usage: cargo run -p ember-server [-- [--inference ADDR] [--params-file PATH] [--style-file PATH] [--log-file PATH]]
//!   --params-file: JSON file with n_predict, temp, top_p, penalty_repeat, mirostat_tau, etc.
//!   Edit between requests to fine-tune output; server reads it on every inference.
//!   Default: inference_params.json
//!
//! # Control pipe
//!
//! The **control pipe** separates app protocol traffic from AI inference. When the Android app sends
//! a control message (`__fetch_push__`, `__get_style__`, `__check_in__`), the server handles it
//! in-process and never forwards it to the LLM.
//!
//! | Message       | Server action                          | Reaches LLM? |
//! |---------------|----------------------------------------|--------------|
//! | `__get_style__`  | Read CSS file, return immediately       | No           |
//! | `__fetch_push__` | Pop from proactive_queue, return payload | No           |
//! | `__check_in__`   | If queue has msg → stream it; else synthetic prompt → LLM | Only synthetic |
//! | `__whatever__`   | Any other __word__ → return empty; no LLM | No           |
//! | User prompt      | format_prompt → gRPC complete_stream    | Yes          |
//!
//! The AI receives `CONTROL_PIPE_INSTRUCTION` in its system prompt. Any `__word__` pattern is a control
//! message: the AI must never say/echo it. The AI can output `__command__` to send control commands;
//! the server strips these from the stream and queues them for the app. The AI can also use
//! `<ember_push>...</ember_push>` for structured payloads.

use std::fs::OpenOptions;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;

use chrono::Utc;
use quinn::{ClientConfig, Endpoint, Incoming, ServerConfig};
use rcgen::{generate_simple_self_signed, CertifiedKey};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::DigitallySignedStruct;
use rustls::crypto::CryptoProvider;
use rustls::{ClientConfig as RustlsClientConfig, SignatureScheme};
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio_stream::{StreamExt, Stream};
use tonic::transport::Channel;
use tonic::{Code, Request};

mod de_kherud_grpc_llm {
    tonic::include_proto!("de.kherud.grpc.llm");
}

use de_kherud_grpc_llm::llm_client::LlmClient;
use de_kherud_grpc_llm::{ChatPrompt, CompleteRequest, CompleteStreamReply, GetStatusRequest, InferenceParameters, Prompt};

/// Inference params from JSON file. All fields optional; missing = use default.
#[derive(serde::Deserialize, Default)]
struct InferenceParamsFile {
    n_predict: Option<i32>,
    temp: Option<f32>,
    top_k: Option<i32>,
    top_p: Option<f32>,
    penalty_last_n: Option<i32>,
    penalty_repeat: Option<f32>,
    mirostat_tau: Option<f32>,
    mirostat_eta: Option<f32>,
}

fn default_params() -> InferenceParameters {
    InferenceParameters {
        n_predict: Some(256),
        temp: Some(0.9),
        top_k: Some(40),
        top_p: Some(0.9),
        penalty_last_n: Some(64),
        penalty_repeat: Some(1.1),
        mirostat_tau: Some(5.0),
        mirostat_eta: Some(0.1),
        ..Default::default()
    }
}

fn load_inference_params(path: &str) -> InferenceParameters {
    let default = default_params();
    let Ok(content) = std::fs::read_to_string(path) else {
        return default;
    };
    let Ok(file) = serde_json::from_str::<InferenceParamsFile>(&content) else {
        return default;
    };
    InferenceParameters {
        n_predict: file.n_predict.or(default.n_predict),
        temp: file.temp.or(default.temp),
        top_k: file.top_k.or(default.top_k),
        top_p: file.top_p.or(default.top_p),
        penalty_last_n: file.penalty_last_n.or(default.penalty_last_n),
        penalty_repeat: file.penalty_repeat.or(default.penalty_repeat),
        mirostat_tau: file.mirostat_tau.or(default.mirostat_tau),
        mirostat_eta: file.mirostat_eta.or(default.mirostat_eta),
        ..Default::default()
    }
}

/// Skip certificate verification for QUIC (development only - do not use in production!)
#[derive(Debug)]
struct SkipServerVerification(Arc<CryptoProvider>);

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self(Arc::new(
            rustls::crypto::ring::default_provider(),
        )))
    }
}

impl ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0
            .signature_verification_algorithms
            .supported_schemes()
    }
}

fn configure_quic_client() -> Result<ClientConfig, Box<dyn std::error::Error + Send + Sync>> {
    let crypto = RustlsClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| format!("{:?}", e))?
    .dangerous()
    .with_custom_certificate_verifier(SkipServerVerification::new())
    .with_no_client_auth();

    let client_config = ClientConfig::new(Arc::new(
        quinn::crypto::rustls::QuicClientConfig::try_from(crypto)?,
    ));
    Ok(client_config)
}

/// gRPC client over QUIC (https) or TCP (http).
enum GrpcClient {
    Tcp(LlmClient<Channel>),
    Quic {
        _endpoint: quinn::Endpoint,
        client: LlmClient<tonic_h3::H3Channel<tonic_h3::quinn::H3QuinnConnector>>,
    },
}

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
    // Load .env from current dir, then from ember workspace (for cargo run from workspace root)
    if dotenvy::dotenv().is_err() {
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            if let Some(ember_dir) = std::path::Path::new(&manifest_dir).parent() {
                let _ = dotenvy::from_path(ember_dir.join(".env"));
            }
        }
    }
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    let mut inference_addr = "https://127.0.0.1:50051".to_string();
    let mut listen_port: u16 = 4433;
    let mut push_port: u16 = 4434;
    let mut log_file: Option<String> = Some("ember-connections.log".to_string());
    let mut style_file = "server/chat-style.css".to_string();
    let mut params_file = "inference_params.json".to_string();
    let mut instructions_file: Option<String> = None;
    let mut web_search = false;
    let mut web_search_always = false;
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--inference" && i + 1 < args.len() {
            inference_addr = args[i + 1].clone();
            i += 2;
            continue;
        }
        if args[i] == "--port" && i + 1 < args.len() {
            if let Ok(p) = args[i + 1].parse() {
                listen_port = p;
            }
            i += 2;
            continue;
        }
        if args[i] == "--push-port" && i + 1 < args.len() {
            if let Ok(p) = args[i + 1].parse() {
                push_port = p;
            }
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
        if args[i] == "--style-file" && i + 1 < args.len() {
            style_file = args[i + 1].clone();
            i += 2;
            continue;
        }
        if args[i] == "--params-file" && i + 1 < args.len() {
            params_file = args[i + 1].clone();
            i += 2;
            continue;
        }
        if args[i] == "--instructions-file" && i + 1 < args.len() {
            instructions_file = Some(args[i + 1].clone());
            i += 2;
            continue;
        }
        if args[i] == "--web-search" {
            web_search = true;
            i += 1;
            continue;
        }
        if args[i] == "--web-search-always" {
            web_search_always = true;
            i += 1;
            continue;
        }
        if args[i] == "--brave-api-key" && i + 1 < args.len() {
            std::env::set_var("BRAVE_API_KEY", &args[i + 1]);
            i += 2;
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

    let listen_addr: SocketAddr = format!("0.0.0.0:{}", listen_port).parse()?;
    let proactive_queue: Arc<tokio::sync::Mutex<Vec<String>>> = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let style_file = Arc::new(style_file);
    let params_file = Arc::new(params_file);
    let brave_key = std::env::var("BRAVE_API_KEY").unwrap_or_default();
    if web_search && brave_key.is_empty() {
        log_err("SERVER", "BRAVE_API_KEY not set; web search disabled. Set it to enable.");
        log_err("SERVER", "Get a key at https://api.search.brave.com");
    }
    let web_search = Arc::new((web_search, web_search_always, brave_key));
    let instructions_file = instructions_file.map(Arc::new);
    run(listen_addr, push_port, inference_addr, conn_log, proactive_queue.clone(), style_file, params_file, instructions_file, web_search)
}

/// Load optional instructions from file. Returns empty string if file missing or unreadable.
fn load_instructions(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default().trim().to_string()
}

/// Background task: every 30 min, generate a proactive check-in and push to queue.
fn spawn_proactive_task(
    grpc_client: Arc<tokio::sync::Mutex<Option<GrpcClient>>>,
    inference_addr: Arc<String>,
    queue: Arc<tokio::sync::Mutex<Vec<String>>>,
    params_file: Arc<String>,
    instructions_file: Option<Arc<String>>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30 * 60));
        interval.tick().await; // skip first immediate tick
        loop {
            interval.tick().await;
            let extra = instructions_file.as_ref().map(|p| load_instructions(p)).unwrap_or_default();
            let llm_prompt = format_prompt("", true, "", false, false, &extra);
            match call_inference_stream(&grpc_client, inference_addr.as_str(), &llm_prompt, params_file.as_str()).await {
                Ok(mut stream) => {
                    let mut text = String::new();
                    while let Some(Ok(reply)) = stream.next().await {
                        text.push_str(&reply.token);
                    }
                    if !text.trim().is_empty() {
                        let mut q = queue.lock().await;
                        q.push(text.trim().to_string());
                        if q.len() > 5 {
                            q.remove(0);
                        }
                        log("PROACTIVE", "queued check-in message");
                    }
                }
                Err(e) => log_err("PROACTIVE", &format!("{e}")),
            }
        }
    });
}

/// Background task: poll grpc_server until ready, then push "Ready for inference!" to queue.
fn spawn_readiness_task(
    grpc_client: Arc<tokio::sync::Mutex<Option<GrpcClient>>>,
    inference_addr: Arc<String>,
    queue: Arc<tokio::sync::Mutex<Vec<String>>>,
) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static READY_SENT: AtomicBool = AtomicBool::new(false);

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
        interval.tick().await; // skip first immediate tick
        loop {
            interval.tick().await;
            if READY_SENT.load(Ordering::Relaxed) {
                continue;
            }
            if let Ok(()) = check_grpc_ready(&grpc_client, inference_addr.as_str()).await {
                READY_SENT.store(true, Ordering::Relaxed);
                let mut q = queue.lock().await;
                q.insert(0, "Ready for inference!".to_string());
                if q.len() > 5 {
                    q.pop();
                }
                log("READY", "inference backend ready; queued 'Ready for inference!' for app");
                break;
            }
        }
    });
}

/// File-based push: poll push-queue.txt, queue lines, then clear. Use when TCP push channel unavailable.
fn spawn_push_file_watcher(queue: Arc<tokio::sync::Mutex<Vec<String>>>) {
    tokio::spawn(async move {
        let path = std::path::Path::new("push-queue.txt");
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            interval.tick().await;
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let lines: Vec<String> = content
                        .lines()
                        .map(|s| s.trim_end_matches('\r').trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    if !lines.is_empty() {
                        let mut guard = queue.lock().await;
                        for line in &lines {
                            guard.push(line.clone());
                            if guard.len() > 10 {
                                guard.remove(0);
                            }
                        }
                        drop(guard);
                        log("PUSH", &format!("from file queued {} line(s)", lines.len()));
                        let _ = std::fs::write(path, "");
                    }
                }
            }
        }
    });
}

/// TCP listener for push channel. External processes connect and send messages (one per line).
/// Messages are queued for the app; app receives them on next __check_in__.
fn spawn_push_listener(push_port: u16, queue: Arc<tokio::sync::Mutex<Vec<String>>>) {
    tokio::spawn(async move {
        let addr: SocketAddr = match format!("0.0.0.0:{}", push_port).parse() {
            Ok(a) => a,
            Err(e) => {
                log_err("PUSH", &format!("invalid push port: {e}"));
                return;
            }
        };
        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                log_err("PUSH", &format!("bind failed: {e}"));
                return;
            }
        };
        log("PUSH", &format!("listening on {} (write messages for app)", addr));
        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    let q = queue.clone();
                    tokio::spawn(async move {
                        let mut buf = String::new();
                        let mut reader = tokio::io::BufReader::new(stream);
                        use tokio::io::AsyncBufReadExt;
                        while let Ok(n) = reader.read_line(&mut buf).await {
                            if n == 0 {
                                break;
                            }
                            let line = buf.trim_end_matches('\n').trim_end_matches('\r').to_string();
                            buf.clear();
                            if line.is_empty() {
                                continue;
                            }
                            let msg = if line.starts_with('{') {
                                serde_json::from_str::<serde_json::Value>(&line)
                                    .ok()
                                    .and_then(|v| v.get("text").and_then(|t| t.as_str()).map(String::from))
                                    .unwrap_or(line)
                            } else {
                                line
                            };
                            if !msg.is_empty() {
                                let mut guard = q.lock().await;
                                guard.push(msg.clone());
                                if guard.len() > 10 {
                                    guard.remove(0);
                                }
                                log("PUSH", &format!("from {} queued ({} chars)", peer, msg.len()));
                            }
                        }
                    });
                }
                Err(e) => log_err("PUSH", &format!("accept error: {e}")),
            }
        }
    });
}

/// Check if grpc_server is ready (model loaded). Creates client if needed.
async fn check_grpc_ready(
    client_opt: &Arc<tokio::sync::Mutex<Option<GrpcClient>>>,
    inference_addr: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut guard = client_opt.lock().await;
    if guard.is_none() {
        let client = if inference_addr.starts_with("https://") {
            let uri: http::Uri = inference_addr
                .parse()
                .map_err(|e| format!("invalid inference URI: {e}"))?;
            let server_name = uri.host().unwrap_or("localhost").to_string();
            let client_config = configure_quic_client()?;
            let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)
                .map_err(|e| format!("QUIC client endpoint: {e}"))?;
            endpoint.set_default_client_config(client_config);
            let connector = tonic_h3::quinn::H3QuinnConnector::new(uri.clone(), server_name, endpoint.clone());
            let channel: tonic_h3::H3Channel<tonic_h3::quinn::H3QuinnConnector> =
                h3_util::client::H3Connection::new(connector, uri);
            GrpcClient::Quic {
                _endpoint: endpoint,
                client: LlmClient::new(channel),
            }
        } else {
            let channel = Channel::from_shared(inference_addr.to_string())?
                .connect()
                .await
                .map_err(|e| format!("gRPC connect failed: {e}"))?;
            GrpcClient::Tcp(LlmClient::new(channel))
        };
        *guard = Some(client);
    }

    let req = Request::new(GetStatusRequest {});
    match guard.as_mut().unwrap() {
        GrpcClient::Tcp(client) => {
            let _ = client.status(req).await.map_err(|e| format!("{e}"))?;
        }
        GrpcClient::Quic { client, .. } => {
            let _ = client.status(req).await.map_err(|e| format!("{e}"))?;
        }
    }
    Ok(())
}

#[tokio::main]
async fn run(
    listen_addr: SocketAddr,
    push_port: u16,
    inference_addr: String,
    conn_log: Option<Arc<std::sync::Mutex<std::fs::File>>>,
    proactive_queue: Arc<tokio::sync::Mutex<Vec<String>>>,
    style_file: Arc<String>,
    params_file: Arc<String>,
    instructions_file: Option<Arc<String>>,
    web_search: Arc<(bool, bool, String)>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let server_config = configure_server()?;
    let endpoint = Endpoint::server(server_config, listen_addr)?;

    log("SERVER", &format!("listening on {}", endpoint.local_addr()?));
    log("SERVER", &format!("push channel: TCP {} (write messages for app)", push_port));
    log("SERVER", &format!("inference: {} (QUIC if https://, TCP if http://)", inference_addr));
    log("SERVER", &format!("params file: {} (edit between requests to tune output)", params_file.as_str()));
    if let Some(ref p) = instructions_file {
        log("SERVER", &format!("instructions file: {} (edit to update behavior in real time)", p.as_str()));
    }
    if web_search.0 {
        log("SERVER", &format!("web search: enabled (Brave{})", if web_search.1 { ", always" } else { "" }));
    }
    log("SERVER", "monitoring all app↔server traffic");

    // Lazy gRPC client - connects on first request so server starts even if Feb17 isn't ready
    let grpc_client: Arc<tokio::sync::Mutex<Option<GrpcClient>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    let inference_addr = Arc::new(inference_addr);

    spawn_proactive_task(grpc_client.clone(), inference_addr.clone(), proactive_queue.clone(), params_file.clone(), instructions_file.clone());
    spawn_readiness_task(grpc_client.clone(), inference_addr.clone(), proactive_queue.clone());
    spawn_push_listener(push_port, proactive_queue.clone());
    spawn_push_file_watcher(proactive_queue.clone());

    while let Some(incoming) = endpoint.accept().await {
        let client_opt = grpc_client.clone();
        let addr = inference_addr.clone();
        let log = conn_log.clone();
        let queue = proactive_queue.clone();
        let style = style_file.clone();
        let params = params_file.clone();
        let instructions = instructions_file.clone();
        let web = web_search.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(incoming, client_opt, addr, log, queue, style, params, instructions, web).await {
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
    grpc_client: Arc<tokio::sync::Mutex<Option<GrpcClient>>>,
    inference_addr: Arc<String>,
    conn_log: Option<Arc<std::sync::Mutex<std::fs::File>>>,
    proactive_queue: Arc<tokio::sync::Mutex<Vec<String>>>,
    style_file: Arc<String>,
    params_file: Arc<String>,
    instructions_file: Option<Arc<String>>,
    web_search: Arc<(bool, bool, String)>,
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
        let queue = proactive_queue.clone();
        let style = style_file.clone();
        let params = params_file.clone();
        let instructions = instructions_file.clone();
        let web = web_search.clone();
        let remote_addr = remote;
        tokio::spawn(async move {
            if let Err(e) = handle_stream(&mut send, &mut recv, client_opt, addr, &log, queue, style, params, instructions, web, remote_addr).await {
                log_err("STREAM", &format!("error: {e}"));
            }
        });
    }

    Ok(())
}

async fn handle_stream(
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
    grpc_client: Arc<tokio::sync::Mutex<Option<GrpcClient>>>,
    inference_addr: Arc<String>,
    conn_log: &Option<Arc<std::sync::Mutex<std::fs::File>>>,
    proactive_queue: Arc<tokio::sync::Mutex<Vec<String>>>,
    style_file: Arc<String>,
    params_file: Arc<String>,
    instructions_file: Option<Arc<String>>,
    web_search: Arc<(bool, bool, String)>,
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
        send_stream_error(send, 0, "What would you like to ask? Type something above and try again.").await?;
        log_connection(conn_log, &remote, "ERROR", "empty prompt");
        return Ok(());
    }

    // ─── Control pipe: app protocol messages are executed here, never forwarded to the LLM ───
    // Any __word__ (exact match) is a control message. Known: __get_style__, __fetch_push__, __check_in__.
    // Unknown __whatever__ → return empty (no LLM).
    if is_control_message(&prompt) {
        let cmd = prompt.trim().to_lowercase();
        if cmd != "__get_style__" && cmd != "__fetch_push__" && cmd != "__check_in__" {
            log("RECV", &format!("control message (unknown): {}", prompt.trim()));
            let interaction_id = {
                use std::sync::atomic::{AtomicU64, Ordering};
                static NEXT_ID: AtomicU64 = AtomicU64::new(0);
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            };
            send.write_all(&stream_frame(json!({
                "type": "stream_start",
                "interaction_id": interaction_id
            }))).await?;
            send.write_all(&stream_frame(json!({
                "type": "stream_end",
                "interaction_id": interaction_id
            }))).await?;
            send.flush().await?;
            send.finish()?;
            return Ok(());
        }
    }

    let is_style_request = prompt.trim().eq_ignore_ascii_case("__get_style__");
    if is_style_request {
        log("RECV", "style request");
        let css = std::fs::read_to_string(style_file.as_str())
            .unwrap_or_else(|_| include_str!("../chat-style.css").to_string());
        send.write_all(&stream_frame(json!({
            "type": "stream_start",
            "interaction_id": 0u64
        }))).await?;
        send.write_all(&stream_frame(json!({
            "type": "stream_token",
            "token": css,
            "interaction_id": 0u64
        }))).await?;
        send.write_all(&stream_frame(json!({
            "type": "stream_end",
            "interaction_id": 0u64
        }))).await?;
        send.flush().await?;
        send.finish()?;
        log("SEND", &format!("style ({} chars)", css.len()));
        return Ok(());
    }

    let is_fetch_push = prompt.trim().eq_ignore_ascii_case("__fetch_push__");
    if is_fetch_push {
        log("RECV", "fetch push (poll for queued messages)");
        let mut q = proactive_queue.lock().await;
        let msg = q.pop().unwrap_or_default();
        drop(q);
        let interaction_id = {
            use std::sync::atomic::{AtomicU64, Ordering};
            static NEXT_ID: AtomicU64 = AtomicU64::new(0);
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        };
        send.write_all(&stream_frame(json!({
            "type": "stream_start",
            "interaction_id": interaction_id
        }))).await?;
        if !msg.is_empty() {
            send.write_all(&stream_frame(json!({
                "type": "stream_token",
                "token": msg,
                "interaction_id": interaction_id
            }))).await?;
        }
        send.write_all(&stream_frame(json!({
            "type": "stream_end",
            "interaction_id": interaction_id
        }))).await?;
        send.flush().await?;
        send.finish()?;
        log("SEND", &format!("fetch_push ({} chars)", msg.len()));
        return Ok(());
    }

    let is_check_in = prompt.trim().eq_ignore_ascii_case("__check_in__");
    if is_check_in {
        log("RECV", "proactive check-in requested");
    }

    let interaction_id = {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    };

    let result = if is_check_in {
        let mut q = proactive_queue.lock().await;
        if let Some(msg) = q.pop() {
            drop(q);
            stream_queued_proactive(send, &msg, interaction_id, conn_log, &remote).await
        } else {
            let extra = instructions_file.as_ref().map(|p| load_instructions(p)).unwrap_or_default();
            let llm_prompt = format_prompt(&prompt, true, "", false, false, &extra);
            stream_inference(send, &grpc_client, &inference_addr, &llm_prompt, interaction_id, conn_log, &remote, &params_file, &proactive_queue).await
        }
    } else {
        let web_ctx = if web_search.0 && !is_context_only_prompt(&prompt) && (web_search.1 || should_search_web(&prompt)) {
            log("BRAVE", &format!("query=\"{}\" → searching", prompt));
            brave_search(&web_search.2, &prompt, 10).await
        } else if web_search.0 && is_context_only_prompt(&prompt) {
            log("BRAVE", &format!("query=\"{}\" → skip (context-only, not a search)", prompt));
            String::new()
        } else if web_search.0 {
            log("BRAVE", &format!("query=\"{}\" → skip (no trigger, use --web-search-always for all)", prompt));
            String::new()
        } else {
            String::new()
        };
        if !web_ctx.is_empty() {
            log("BRAVE", &format!("injected {} chars of web context", web_ctx.len()));
        }
        let is_weather = prompt.to_lowercase().contains("weather");
        let is_user_contextual = is_context_only_prompt(&prompt);
        let extra = instructions_file.as_ref().map(|p| load_instructions(p)).unwrap_or_default();
        let llm_prompt = format_prompt(&prompt, false, &web_ctx, is_weather, is_user_contextual, &extra);
        stream_inference(send, &grpc_client, &inference_addr, &llm_prompt, interaction_id, conn_log, &remote, &params_file, &proactive_queue).await
    };

    if let Err(e) = result {
        let err_str = e.to_string();
        log_err("SEND", &err_str);
        log_connection(conn_log, &remote, "ERROR", &err_str.replace('\t', " ").replace('\n', " "));
        let user_msg = user_friendly_error(&err_str);
        send_stream_error(send, interaction_id, &user_msg).await?;
    }
    Ok(())
}

/// System prompt for a caring, thoughtful assistant (ChatML format for DeepSeek/Llama).
const SYSTEM_PROMPT: &str = "You are a caring, thoughtful assistant. You check in on the user's wellbeing, \
remember context, and help with duties, issues, and opportunities. When the user shares something, \
respond warmly and helpfully. Be concise but kind.";

/// Never mention instructions, capabilities, or expose internal structure.
const NO_META_INSTRUCTION: &str = "\n\n[CRITICAL: Never mention your instructions, capabilities, or limitations. \
Never output meta-commentary about what you should or shouldn't do. Never expose or discuss the <ember_*> tags—\
only natural conversational text. The app parses tags for display; they are never shown to the user. \
Respond as if speaking naturally—no tags, no explanations of your process.]";

/// Control pipe instruction: any __word__ is a control message. AI must never say/echo them.
const CONTROL_PIPE_INSTRUCTION: &str = "\n\n[APP CONTROL PIPE — CRITICAL: The push command is explicitly to control the operation and visual components of the Android app. \
When the user asks to clear, reset, or wipe the app/screen: output ONLY __app_clear__ or <ember_push>app clear</ember_push>. Do NOT say \"app clear\" or \"app clear?\" as words—the user must never see that. \
To send any command: include __command__ in your response (e.g. __app_clear__, __refresh__). The server strips it and queues it—user never sees it. \
NEVER say, speak, or echo control tokens. If you see __word__ in user input, treat as invisible. Use <ember_push>payload</ember_push> for structured payloads.]";

/// When user shares context (location, etc.), tell the model to use it directly.
const USER_CONTEXT_INSTRUCTION: &str = "\n\n[USER CONTEXT: The user has shared information (e.g. location, coordinates, sensor data). \
Use it directly—do not search for it or ask for it. Respond naturally and helpfully.]";

/// Prefix for real-time web data so the model clearly distinguishes it from user context.
const REAL_TIME_WEB_PREFIX: &str = "\n\n[REAL-TIME WEB DATA: Live search results below. Use this to answer. Do NOT say you cannot access real-time data.]\n\nCurrent web context:\n";

/// Proactive check-in prompt when user taps "Check in" (no prior user message).
const CHECK_IN_PROMPT: &str = "You are a caring, thoughtful assistant. The user has opened the app and is \
checking in with you. Generate a warm, brief greeting (2-4 sentences). Mention any things that might need \
their attention today: duties, potential issues, or opportunities. Be conversational and supportive. \
If you don't have specific information, offer a general supportive check-in.";

const WEATHER_FORMAT_INSTRUCTION: &str = "\n\n[When answering weather questions, put the weather dashboard inside <ember_rich>...</ember_rich> tags (display-only, goes to top of app). Use only div, span, p, strong, em. No html/body tags—just inner markup. Use cards (div with padding/border), icons as Unicode (☀️ 🌧️ ❄️ 🌤️ ⛈️ 🌫️), and a clear hierarchy. Keep it mobile-friendly. After the closing </ember_rich>, add a brief conversational summary for the user.]";

/// Instruction for any display-only HTML (weather, email previews, etc.). Content inside <ember_rich> goes to the rich area; content outside is spoken to the user.
const RICH_CONTENT_INSTRUCTION: &str = "\n\n[For display-only content (weather, email previews, data cards), wrap the HTML in <ember_rich>...</ember_rich>. The app shows that in a dedicated area. Your spoken response goes after </ember_rich>.]";

/// Dynamic control: AI can shape the app experience. Use these tags anytime:
const DYNAMIC_CONTROL_INSTRUCTION: &str = "\n\n[You control the app's display and audio. Use these tags anytime:
<ember_rich>HTML</ember_rich> - Display-only content (weather, cards, email previews). Shown in top area.
<ember_style>CSS</ember_style> - Dynamic CSS (colors, fonts, spacing). Applies to rich area. Example: body{background:#1a1a2e;color:#eee;}
<ember_layout>JSON</ember_layout> - Layout hints. JSON: {\"rich_height\":\"full\"|\"auto\"|\"140\", \"theme\":\"dark\"|\"light\"|\"warm\"}
<ember_speak>text</ember_speak> - Speak this via TTS (e.g. alerts, emphasis). Use for important updates.
<ember_push>payload</ember_push> - Explicitly controls the operation and visual components of the Android app. Payload: \"app clear\" or JSON {\"chat\":[...], \"rich\":\"...\", \"layout\":{...}}. The app receives it on its next poll. Use when the user asks to clear, reset, or update the app display. Never show this tag to the user—it is processed server-side.
__word__ - Any double-underscore token (e.g. __app_clear__, __refresh__) is sent to the control pipe and never shown to the user. Output __command__ to issue control; never say it aloud.
Vary styles and layouts to match context: weather→warm tones, news→editorial, calm→soft. Provide rich, accurate info from web context.]";

fn format_prompt(user_msg: &str, is_check_in: bool, web_context: &str, is_weather: bool, is_user_contextual: bool, extra_instructions: &str) -> String {
    let (system, user_part) = if is_check_in {
        (CHECK_IN_PROMPT, "The user is checking in.")
    } else {
        (SYSTEM_PROMPT, user_msg)
    };
    let mut system_with_web = if web_context.is_empty() {
        system.to_string()
    } else {
        format!("{system}{web_context}")
    };
    if is_user_contextual {
        system_with_web.push_str(USER_CONTEXT_INSTRUCTION);
    }
    if is_weather {
        system_with_web.push_str(WEATHER_FORMAT_INSTRUCTION);
    } else if !web_context.is_empty() {
        system_with_web.push_str(RICH_CONTENT_INSTRUCTION);
    }
    system_with_web.push_str(DYNAMIC_CONTROL_INSTRUCTION);
    system_with_web.push_str(CONTROL_PIPE_INSTRUCTION);
    system_with_web.push_str(NO_META_INSTRUCTION);
    if !extra_instructions.is_empty() {
        system_with_web.push_str("\n\n");
        system_with_web.push_str(extra_instructions);
    }
    format!(
        "<|im_start|>system\n{system_with_web}<|im_end|>\n<|im_start|>user\n{user_part}<|im_end|>\n<|im_start|>assistant\n"
    )
}

/// Brave Search API response (partial).
#[derive(serde::Deserialize)]
struct BraveResponse {
    web: Option<BraveWeb>,
}

#[derive(serde::Deserialize)]
struct BraveWeb {
    results: Option<Vec<BraveResult>>,
}

#[derive(serde::Deserialize)]
struct BraveResult {
    title: Option<String>,
    url: Option<String>,
    description: Option<String>,
}

/// Brave Search: returns web context string for prompt injection.
async fn brave_search(api_key: &str, query: &str, count: u32) -> String {
    if api_key.is_empty() {
        log_err("BRAVE", "API key empty, skipping");
        return String::new();
    }
    let count = count.min(20);
    let url = match reqwest::Url::parse_with_params(
        "https://api.search.brave.com/res/v1/web/search",
        &[("q", query), ("count", &count.to_string())],
    ) {
        Ok(u) => u,
        Err(e) => {
            log_err("BRAVE", &format!("invalid URL: {e}"));
            return String::new();
        }
    };
    log("BRAVE", &format!("GET {} query=\"{}\"", url, query));
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            log_err("BRAVE", &format!("client build failed: {e}"));
            return String::new();
        }
    };
    let resp = match client
        .get(url.as_str())
        .header("X-Subscription-Token", api_key)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            log_err("BRAVE", &format!("request failed: {e}"));
            return String::new();
        }
    };
    let status = resp.status();
    if !status.is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        log_err("BRAVE", &format!("status {} body={}", status, body_text));
        return String::new();
    }
    log("BRAVE", &format!("response status {}", status));
    let data: BraveResponse = match resp.json().await {
        Ok(d) => d,
        Err(e) => {
            log_err("BRAVE", &format!("parse failed: {e}"));
            return String::new();
        }
    };
    let Some(web) = data.web else {
        log("BRAVE", "no web results in response");
        return String::new();
    };
    let Some(results) = web.results else {
        log("BRAVE", "no results in response");
        return String::new();
    };
    log("BRAVE", &format!("got {} results", results.len()));
    for (i, r) in results.iter().enumerate() {
        let title = r.title.as_deref().unwrap_or("(no title)");
        let url = r.url.as_deref().unwrap_or("");
        log("BRAVE", &format!("  [{}] {} {}", i + 1, title, url));
    }
    let mut ctx = String::from(REAL_TIME_WEB_PREFIX);
    for (i, r) in results.iter().enumerate().take(count as usize) {
        let desc = r.description.as_deref().unwrap_or("").trim();
        if !desc.is_empty() {
            let title = r.title.as_deref().unwrap_or("(no title)");
            let url = r.url.as_deref().unwrap_or("");
            let snippet = desc.chars().take(400).collect::<String>();
            ctx.push_str(&format!("{}. {} ({})\n   {}\n\n", i + 1, title, url, snippet));
        }
    }
    ctx.push_str("]\n\n");
    ctx
}

/// Heuristic: is this app-provided context (location, sensor data) rather than a user question?
/// Skip Brave search for these — they don't benefit from web results.
fn is_context_only_prompt(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    let context_patterns = [
        "my location:",
        "location: lat",
        "lat ",
        "lon ",
        "accuracy ~",
        "accuracy:",
        "coordinates:",
        "gps:",
        "sensor:",
        "battery:",
        "__get_style__",
        "__check_in__",
        "__fetch_push__",
    ];
    context_patterns.iter().any(|p| lower.contains(p))
}

/// Heuristic: does the prompt likely need real-time web info?
fn should_search_web(prompt: &str) -> bool {
    if is_context_only_prompt(prompt) {
        return false;
    }
    let lower = prompt.to_lowercase();
    let triggers = [
        // Time-sensitive
        "weather", "news", "latest", "today", "current", "now", "recent", "right now",
        "what is happening", "what's happening", "what happened", "breaking", "headlines",
        // Finance & markets
        "price", "stock", "bitcoin", "crypto", "ethereum", "market", "dollar", "euro",
        // Sports & events
        "score", "game", "match", "election", "today's", "championship", "world cup",
        // Factual / lookup
        "who is", "who's", "who won", "when did", "where is", "how much", "what is the",
        "definition of", "meaning of", "capital of", "population of", "time in", "date",
        // Year/recency hints
        "2024", "2025", "this year", "this month", "this week",
    ];
    triggers.iter().any(|t| lower.contains(t))
}

/// Map gRPC/connection errors to a user-friendly message for the Android app.
fn user_friendly_error(err: &str) -> String {
    let lower = err.to_lowercase();
    if lower.contains("transport error")
        || lower.contains("connection refused")
        || lower.contains("connection reset")
        || lower.contains("connect failed")
        || lower.contains("unreachable")
    {
        "The AI is still warming up. Give it a minute or two and try again.".to_string()
    } else if lower.contains("timeout") || lower.contains("timed out") {
        "That took a bit too long. Try again in a moment.".to_string()
    } else if lower.contains("model is still loading") || lower.contains("not ready") {
        "The AI is still warming up. Give it a minute or two and try again.".to_string()
    } else if lower.contains("not found") || lower.contains("404") {
        "Something went wrong on the server. Try again later.".to_string()
    } else if lower.contains("unavailable") {
        "The AI isn't available right now. Try again in a moment.".to_string()
    } else {
        "Something went wrong. Try again in a moment.".to_string()
    }
}

/// Returns true if s is exactly __word__ (alphanumeric + underscore between double underscores).
fn is_control_message(s: &str) -> bool {
    let s = s.trim();
    if s.len() < 4 || !s.starts_with("__") || !s.ends_with("__") {
        return false;
    }
    let inner = &s[2..s.len() - 2];
    !inner.is_empty() && inner.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Extract __word__ tokens and "app clear" phrase from s, remove them, return (stripped_string, control_tokens).
fn extract_control_tokens(s: &str) -> (String, Vec<String>) {
    let mut out = s.to_string();
    let mut control = Vec::new();
    // Failsafe: AI sometimes says "app clear" or "app clear?" as text—strip and queue so app actually resets
    let lower = out.to_lowercase();
    if lower.contains("app clear") {
        if let Some(pos) = lower.find("app clear") {
            let end = pos + 9;
            let suffix_len = out[end..].chars().take_while(|c| matches!(*c, '?' | '!' | '.' | ' ' | '\n' | ',')).count();
            out = format!("{}{}", &out[..pos], &out[end + suffix_len..]);
            control.push("app clear".to_string());
        }
    }
    loop {
        let Some(open) = out.find("__") else { break };
        let after = open + 2;
        if after >= out.len() {
            break;
        }
        let rest = &out[after..];
        let mut end = 0usize;
        let mut found = false;
        for (i, c) in rest.char_indices() {
            if c == '_' && rest.get(i..).map_or(false, |t| t.starts_with("__")) {
                end = i;
                found = true;
                break;
            }
            if !c.is_ascii_alphanumeric() && c != '_' {
                break;
            }
        }
        if found && end > 0 {
            let word = format!("__{}__", &rest[..end]);
            control.push(word.clone());
            out = format!("{}{}", &out[..open], &rest[end + 2..]);
        } else {
            break;
        }
    }
    (out, control)
}

/// Strip any leaked tags or meta-markup from chat tokens so they never appear in the UI.
fn sanitize_chat_token(s: &str) -> String {
    let mut out = s.to_string();
    for tag in [
        "<ember_rich>", "</ember_rich>",
        "<ember_style>", "</ember_style>",
        "<ember_layout>", "</ember_layout>",
        "<ember_speak>", "</ember_speak>",
        "<ember_push>", "</ember_push>",
    ] {
        out = out.replace(tag, "");
    }
    // Strip __word__ control tokens (never show to user)
    out = extract_control_tokens(&out).0;
    // Strip any <|...|> tokens (ChatML, etc.)
    while let Some(start) = out.find("<|") {
        if let Some(end) = out[start..].find("|>") {
            out = format!("{}{}", &out[..start], &out[start + end + 2..]);
        } else {
            break;
        }
    }
    out
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
    send.flush().await?;
    send.finish()?;
    Ok(())
}

/// Send a queued proactive message as a stream (no LLM call).
async fn stream_queued_proactive(
    send: &mut quinn::SendStream,
    msg: &str,
    interaction_id: u64,
    conn_log: &Option<Arc<std::sync::Mutex<std::fs::File>>>,
    remote: &SocketAddr,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    send.write_all(&stream_frame(json!({
        "type": "stream_start",
        "interaction_id": interaction_id
    }))).await?;
    if !msg.is_empty() {
        send.write_all(&stream_frame(json!({
            "type": "stream_token",
            "token": msg,
            "interaction_id": interaction_id
        }))).await?;
    }
    send.write_all(&stream_frame(json!({
        "type": "stream_end",
        "interaction_id": interaction_id
    }))).await?;
    send.flush().await?;
    send.finish()?;
    log("SEND", &format!("proactive (queued, {} chars)", msg.len()));
    log_connection(conn_log, remote, "RESPONSE", &format!("response_len={} (queued)", msg.len()));
    Ok(())
}

async fn stream_inference(
    send: &mut quinn::SendStream,
    grpc_client: &Arc<tokio::sync::Mutex<Option<GrpcClient>>>,
    inference_addr: &Arc<String>,
    prompt: &str,
    interaction_id: u64,
    conn_log: &Option<Arc<std::sync::Mutex<std::fs::File>>>,
    remote: &SocketAddr,
    params_file: &str,
    proactive_queue: &Arc<tokio::sync::Mutex<Vec<String>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // stream_start
    let frame = stream_frame(json!({
        "type": "stream_start",
        "interaction_id": interaction_id
    }));
    send.write_all(&frame).await?;

    let mut total_len = 0usize;
    let mut total_rich = 0usize;
    let mut stream = call_inference_stream(grpc_client, inference_addr, prompt, params_file).await?;

    let mut buf = String::new();
    let mut inside_block: Option<(&str, &str)> = None; // (open_tag, close_tag)
    const BLOCKS: &[(&str, &str, &str)] = &[
        ("<ember_push>", "</ember_push>", "queue_push"),
        ("<ember_rich>", "</ember_rich>", "stream_rich"),
        ("<ember_style>", "</ember_style>", "stream_style"),
        ("<ember_layout>", "</ember_layout>", "stream_layout"),
        ("<ember_speak>", "</ember_speak>", "stream_audio"),
    ];
    const HOLD_BACK: usize = 20; // hold back to avoid splitting "app clear" or tags across flushes

    while let Some(result) = stream.next().await {
        match result {
            Ok(reply) => {
                let mut token = reply.token;
                for tag in ["<|im_end|>", "<|im_start|>", "<|end|>", "<|start|>"] {
                    token = token.replace(tag, "");
                }
                while let Some(start) = token.find("<|") {
                    if let Some(end) = token[start..].find("|>") {
                        token = format!("{}{}", &token[..start], &token[start + end + 2..]);
                    } else {
                        break;
                    }
                }
                if token.is_empty() {
                    continue;
                }
                buf.push_str(&token);

                loop {
                    let (flush_before, found_block) = if let Some((open, close)) = inside_block {
                        if let Some(close_pos) = buf.find(close) {
                            let content = buf[..close_pos].trim();
                            let frame_type = BLOCKS.iter().find(|(o, c, _)| *o == open && *c == close).map(|(_, _, t)| *t).unwrap_or("stream_rich");
                            if !content.is_empty() {
                                if frame_type == "queue_push" {
                                    let mut q = proactive_queue.lock().await;
                                    q.push(content.to_string());
                                    if q.len() > 5 {
                                        q.remove(0);
                                    }
                                    drop(q);
                                    log("PUSH", &format!("→ queued control ({} chars)", content.len()));
                                } else {
                                    if frame_type == "stream_rich" {
                                        total_rich += content.len();
                                        log("RICH", &format!("→ rich area ({} chars)", content.len()));
                                    } else {
                                        log("CTRL", &format!("→ {} ({} chars)", frame_type, content.len()));
                                    }
                                    let frame = match frame_type {
                                        "stream_style" => stream_frame(json!({"type":"stream_style","css":content,"interaction_id":interaction_id})),
                                        "stream_layout" => stream_frame(json!({"type":"stream_layout","json":content,"interaction_id":interaction_id})),
                                        "stream_audio" => stream_frame(json!({"type":"stream_audio","text":content,"interaction_id":interaction_id})),
                                        _ => stream_frame(json!({"type":"stream_rich","content":content,"interaction_id":interaction_id})),
                                    };
                                    send.write_all(&frame).await?;
                                }
                            }
                            buf = buf[close_pos + close.len()..].to_string();
                            inside_block = None;
                            (None, true)
                        } else {
                            (None, false)
                        }
                    } else {
                        let mut found = None;
                        for (open, close, _) in BLOCKS {
                            if let Some(open_pos) = buf.find(open) {
                                let before_buf = &buf[..open_pos];
                                let (before_stripped, ctrl_tokens) = extract_control_tokens(before_buf);
                                for t in ctrl_tokens {
                                    let mut q = proactive_queue.lock().await;
                                    q.push(t.clone());
                                    if q.len() > 5 {
                                        q.remove(0);
                                    }
                                    drop(q);
                                    log("PUSH", &format!("→ queued __control__ ({} chars)", t.len()));
                                }
                                let before = sanitize_chat_token(&before_stripped);
                                if !before.is_empty() {
                                    total_len += before.len();
                                    let f = stream_frame(json!({"type":"stream_token","token":before,"interaction_id":interaction_id}));
                                    send.write_all(&f).await?;
                                }
                                buf = buf[open_pos + open.len()..].to_string();
                                inside_block = Some((open, close));
                                found = Some(());
                                break;
                            }
                        }
                        if found.is_some() {
                            (None, true)
                        } else {
                            let mut safe_len = buf.len().saturating_sub(HOLD_BACK);
                            // Don't split "app clear" across flushes—trim flush so it doesn't end with a prefix
                            if safe_len > 0 {
                                let flush = &buf[..safe_len];
                                for prefix in ["app clear?", "app clear!", "app clear.", "app clear ", "app clear", "app cle", "app cl", "app c", "app ", "app", "ap", "a"] {
                                    if flush.ends_with(prefix) {
                                        safe_len = flush.len() - prefix.len();
                                        break;
                                    }
                                }
                            }
                            if safe_len > 0 {
                                let flush = buf[..safe_len].to_string();
                                buf = buf[safe_len..].to_string();
                                (Some(flush), false)
                            } else {
                                (None, false)
                            }
                        }
                    };

                    if let Some(flush) = flush_before {
                        let (stripped, ctrl_tokens) = extract_control_tokens(&flush);
                        for t in ctrl_tokens {
                            let mut q = proactive_queue.lock().await;
                            q.push(t.clone());
                            if q.len() > 5 {
                                q.remove(0);
                            }
                            drop(q);
                            log("PUSH", &format!("→ queued __control__ ({} chars)", t.len()));
                        }
                        let sanitized = sanitize_chat_token(&stripped);
                        if !sanitized.is_empty() {
                            total_len += sanitized.len();
                            let f = stream_frame(json!({"type":"stream_token","token":sanitized,"interaction_id":interaction_id}));
                            send.write_all(&f).await?;
                        }
                    }
                    if !found_block {
                        break;
                    }
                }
            }
            Err(e) => return Err(format!("inference stream error: {e}").into()),
        }
    }

    // Flush remaining: if inside_rich, treat as chat (incomplete block); else chat
    if !buf.is_empty() {
        let (stripped, ctrl_tokens) = extract_control_tokens(&buf);
        for t in ctrl_tokens {
            let mut q = proactive_queue.lock().await;
            q.push(t.clone());
            if q.len() > 5 {
                q.remove(0);
            }
            drop(q);
            log("PUSH", &format!("→ queued __control__ ({} chars)", t.len()));
        }
        let sanitized = sanitize_chat_token(&stripped);
        if !sanitized.is_empty() {
            total_len += sanitized.len();
            let frame = stream_frame(json!({
                "type": "stream_token",
                "token": sanitized,
                "interaction_id": interaction_id
            }));
            send.write_all(&frame).await?;
        }
    }

    // stream_end
    let frame = stream_frame(json!({
        "type": "stream_end",
        "interaction_id": interaction_id
    }));
    send.write_all(&frame).await?;
    send.flush().await?;
    send.finish()?;

    log("SEND", &format!("response stream (chat={} rich={})", total_len, total_rich));
    log_connection(conn_log, remote, "RESPONSE", &format!("response_len={} rich={}", total_len, total_rich));
    Ok(())
}

type InferenceStream = std::pin::Pin<Box<dyn Stream<Item = Result<CompleteStreamReply, tonic::Status>> + Send>>;

/// Convert a full response into a token stream for real-time UX when the backend
/// doesn't support streaming. Chunks by words so the client receives progressive updates.
fn full_response_to_stream(response: String) -> InferenceStream {
    let tokens: Vec<CompleteStreamReply> = response
        .split_whitespace()
        .map(|s| CompleteStreamReply {
            token: format!("{s} "),
        })
        .collect();
    let stream = tokio_stream::iter(tokens.into_iter().map(|r| Ok::<_, tonic::Status>(r)));
    Box::pin(stream)
}

async fn call_inference_stream(
    client_opt: &Arc<tokio::sync::Mutex<Option<GrpcClient>>>,
    inference_addr: &str,
    prompt: &str,
    params_file: &str,
) -> Result<InferenceStream, Box<dyn std::error::Error + Send + Sync>> {
    let use_quic = inference_addr.starts_with("https://");

    let mut guard = client_opt.lock().await;
    if guard.is_none() {
        let client = if use_quic {
            let uri: http::Uri = inference_addr
                .parse()
                .map_err(|e| format!("invalid inference URI: {e}"))?;
            let server_name = uri
                .host()
                .unwrap_or("localhost")
                .to_string();
            let client_config = configure_quic_client()?;
            let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)
                .map_err(|e| format!("QUIC client endpoint: {e}"))?;
            endpoint.set_default_client_config(client_config);
            let connector = tonic_h3::quinn::H3QuinnConnector::new(
                uri.clone(),
                server_name,
                endpoint.clone(),
            );
            let channel: tonic_h3::H3Channel<tonic_h3::quinn::H3QuinnConnector> =
                h3_util::client::H3Connection::new(connector, uri);
            let client = LlmClient::new(channel);
            log("GRPC", "QUIC client ready");
            GrpcClient::Quic {
                _endpoint: endpoint,
                client,
            }
        } else {
            let channel = Channel::from_shared(inference_addr.to_string())?
                .connect()
                .await
                .map_err(|e| format!("gRPC connect failed: {e}"))?;
            log("GRPC", "TCP client ready");
            GrpcClient::Tcp(LlmClient::new(channel))
        };
        *guard = Some(client);
    }

    let params = load_inference_params(params_file);
    let complete_req = CompleteRequest {
        prompt: Some(Prompt {
            prompt: Some(de_kherud_grpc_llm::prompt::Prompt::Chat(ChatPrompt {
                prompt: prompt.to_string(),
            })),
        }),
        parameters: Some(params),
    };

    let req = Request::new(complete_req.clone());

    match guard.as_mut().unwrap() {
        GrpcClient::Tcp(client) => run_complete_stream_tcp(client, complete_req, req).await,
        GrpcClient::Quic { client, .. } => run_complete_stream_quic(client, complete_req, req).await,
    }
}

async fn run_complete_stream_tcp(
    client: &mut LlmClient<Channel>,
    complete_req: CompleteRequest,
    req: Request<CompleteRequest>,
) -> Result<InferenceStream, Box<dyn std::error::Error + Send + Sync>> {
    match client.complete_stream(req).await {
        Ok(resp) => Ok(Box::pin(resp.into_inner())),
        Err(e) if e.code() == Code::Unimplemented => {
            log("GRPC", "complete_stream UNIMPLEMENTED, falling back to complete (chunked streaming)");
            let reply = client
                .complete(Request::new(complete_req))
                .await
                .map_err(|e| format!("inference complete failed: {e}"))?
                .into_inner();
            Ok(full_response_to_stream(reply.response))
        }
        Err(e) => Err(format!("inference stream failed: {e}").into()),
    }
}

async fn run_complete_stream_quic(
    client: &mut LlmClient<tonic_h3::H3Channel<tonic_h3::quinn::H3QuinnConnector>>,
    complete_req: CompleteRequest,
    req: Request<CompleteRequest>,
) -> Result<InferenceStream, Box<dyn std::error::Error + Send + Sync>> {
    match client.complete_stream(req).await {
        Ok(resp) => Ok(Box::pin(resp.into_inner())),
        Err(e) if e.code() == Code::Unimplemented => {
            log("GRPC", "complete_stream UNIMPLEMENTED, falling back to complete (chunked streaming)");
            let reply = client
                .complete(Request::new(complete_req))
                .await
                .map_err(|e| format!("inference complete failed: {e}"))?
                .into_inner();
            Ok(full_response_to_stream(reply.response))
        }
        Err(e) => Err(format!("inference stream failed: {e}").into()),
    }
}
