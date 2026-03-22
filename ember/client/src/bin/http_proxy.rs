//! HTTP/HTTPS proxy for the Flutter web app.
//! Browsers cannot use QUIC; this proxy accepts HTTP(S) and forwards to ember over QUIC.
//!
//! Usage: cargo run --bin http_proxy -p ember-client --features http_proxy [-- OPTIONS]
//!
//! Options:
//!   --port PORT     Listen port (default: 8443 for HTTPS, 8080 for HTTP)
//!   --cert PATH    Certificate PEM file (required for HTTPS unless --http)
//!   --key PATH     Private key PEM file (required for HTTPS unless --http)
//!   --http         Use plain HTTP instead of HTTPS (development only)
//!
//! If --cert and --key are omitted and not --http, generates a self-signed cert.
//!
//! POST /ask
//! Body: {"server":"host:port","prompt":"..."}
//! Response: plain text (the ember response)
//!
//! POST /ask/stream
//! Body: {"server":"host:port","prompt":"..."}
//! Response: Server-Sent Events (SSE) stream of tokens for low-latency display

use axum::{
    extract::Json,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Router,
};
use axum::response::sse::{Event, Sse};
use axum_server::tls_rustls::RustlsConfig;
use rcgen::generate_simple_self_signed;
use serde::Deserialize;
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tower_http::cors::{Any, CorsLayer};

#[derive(Deserialize)]
struct AskRequest {
    server: String,
    prompt: String,
}

async fn ask_handler(Json(req): Json<AskRequest>) -> impl IntoResponse {
    let server = req.server.clone();
    let prompt = req.prompt.clone();
    let result = tokio::task::spawn_blocking(move || ember_native::ask_ai(&server, &prompt))
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
    match result {
        Ok(response) => (StatusCode::OK, response),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error: {}", e),
        ),
    }
}

async fn ask_stream_handler(Json(req): Json<AskRequest>) -> impl IntoResponse {
    let (tx, rx) = mpsc::channel::<String>(64);
    let server = req.server.clone();
    let prompt = req.prompt.clone();

    tokio::task::spawn_blocking(move || {
        let _ = ember_native::ask_ai_streaming(&server, &prompt, |token| {
            let _ = tx.blocking_send(token.to_string());
        });
    });

    let stream = ReceiverStream::new(rx).map(|token| Ok::<_, Infallible>(Event::default().data(token)));
    Sse::new(stream)
}

fn parse_args() -> (u16, Option<String>, Option<String>, bool) {
    let mut port: u16 = 8443;
    let mut cert: Option<String> = None;
    let mut key: Option<String> = None;
    let mut http = false;

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--port" && i + 1 < args.len() {
            if let Ok(p) = args[i + 1].parse() {
                port = p;
            }
            i += 2;
            continue;
        }
        if args[i] == "--cert" && i + 1 < args.len() {
            cert = Some(args[i + 1].clone());
            i += 2;
            continue;
        }
        if args[i] == "--key" && i + 1 < args.len() {
            key = Some(args[i + 1].clone());
            i += 2;
            continue;
        }
        if args[i] == "--http" {
            http = true;
            if port == 8443 {
                port = 8080;
            }
            i += 1;
            continue;
        }
        i += 1;
    }
    (port, cert, key, http)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    let (port, cert_path, key_path, use_http) = parse_args();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .route("/ask", post(ask_handler))
        .route("/ask/stream", post(ask_stream_handler))
        .layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));

    if use_http {
        println!("HTTP proxy (plain) listening on http://{}", addr);
        println!("WARNING: Traffic is unencrypted. Use HTTPS for production.");
        axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    } else {
        let tls_config = match (cert_path, key_path) {
            (Some(cert), Some(key)) => {
                RustlsConfig::from_pem_file(&cert, &key).await?
            }
            _ => {
                let names = vec![
                    "localhost".to_string(),
                    "127.0.0.1".to_string(),
                ];
                let certified = generate_simple_self_signed(names)?;
                let cert_pem = certified.cert.pem();
                let key_pem = certified.key_pair.serialize_pem();
                RustlsConfig::from_pem(cert_pem.into_bytes(), key_pem.into_bytes()).await?
            }
        };
        println!("HTTPS proxy listening on https://{}", addr);
        println!("POST /ask with {{\"server\":\"host:port\",\"prompt\":\"...\"}}");
        println!("POST /ask/stream for SSE streaming (low latency)");
        axum_server::bind_rustls(addr, tls_config)
            .serve(app.into_make_service())
            .await?;
    }
    Ok(())
}
