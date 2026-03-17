//! Ember QUIC client library. Used by the desktop binary and Android JNI.

#[cfg(feature = "android")]
mod jni;

#[cfg(feature = "ios")]
mod ios;

use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::Arc;

use quinn::{ClientConfig, Endpoint};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::crypto::CryptoProvider;
use rustls::{ClientConfig as RustlsClientConfig, DigitallySignedStruct, SignatureScheme};

/// Skip certificate verification (development only - do not use in production!)
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

fn configure_client(provider: Arc<CryptoProvider>) -> Result<ClientConfig, Box<dyn std::error::Error + Send + Sync>> {
    let crypto = RustlsClientConfig::builder_with_provider(provider)
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

/// Ask the AI a question via the ember server (which forwards to Feb17 inference).
/// Uses hostname for TLS SNI when addr_str is "host:port" (e.g. pinggy URLs).
pub fn ask_ai(addr_str: &str, prompt: &str) -> Result<String, String> {
    ask_ai_streaming(addr_str, prompt, |_| {})
}

/// Ask the AI (with pre-resolved address; uses IP for SNI).
pub fn ask_ai_addr(server_addr: SocketAddr, prompt: &str) -> Result<String, String> {
    ask_ai_addr_streaming(server_addr, prompt, |_| {})
}

/// Ask the AI with a callback for each token (for progressive UI display).
/// Uses hostname from addr_str for TLS SNI (required for pinggy/proxy connections).
pub fn ask_ai_streaming<F>(addr_str: &str, prompt: &str, on_token: F) -> Result<String, String>
where
    F: FnMut(&str) + Send,
{
    ask_ai_streaming_with_rich(addr_str, prompt, on_token, |_| {})
}

/// Ask the AI with callbacks for chat tokens and rich HTML (weather, email previews).
pub fn ask_ai_streaming_with_rich<F, G>(addr_str: &str, prompt: &str, on_token: F, on_rich: G) -> Result<String, String>
where
    F: FnMut(&str) + Send,
    G: FnMut(&str) + Send,
{
    ask_ai_streaming_full(addr_str, prompt, on_token, on_rich, |_| {}, |_| {}, |_| {}, |_| {})
}

/// Ask the AI with full control callbacks: token, rich, style, layout, audio, control_payload.
pub fn ask_ai_streaming_full<F, G, H, I, J, K>(
    addr_str: &str, prompt: &str,
    on_token: F, on_rich: G, on_style: H, on_layout: I, on_audio: J, on_control: K,
) -> Result<String, String>
where
    F: FnMut(&str) + Send,
    G: FnMut(&str) + Send,
    H: FnMut(&str) + Send,
    I: FnMut(&str) + Send,
    J: FnMut(&str) + Send,
    K: FnMut(&str) + Send,
{
    let addrs: Vec<SocketAddr> = addr_str.to_socket_addrs().map_err(|e| {
        let msg = e.to_string();
        if msg.contains("lookup") || msg.contains("hostname") || msg.contains("no address") {
            "Could not resolve hostname. Check: (1) proxy reachable (e.g. pinggy), (2) device has internet, (3) hostname is correct.".to_string()
        } else {
            msg
        }
    })?.collect();
    let server_addr = addrs
        .iter()
        .find(|a| a.is_ipv4())
        .copied()
        .or_else(|| addrs.into_iter().next())
        .ok_or_else(|| "Could not resolve address".to_string())?;
    let server_name = extract_host(addr_str);
    ask_ai_addr_streaming_full(server_addr, server_name, prompt, on_token, on_rich, on_style, on_layout, on_audio, on_control)
}

/// Extract host part from "host:port" for TLS SNI.
fn extract_host(addr_str: &str) -> String {
    addr_str.rsplit_once(':')
        .map(|(host, _)| host.to_string())
        .unwrap_or_else(|| addr_str.to_string())
}

/// Ask the AI with pre-resolved address (uses IP for SNI; for direct IP connections).
pub fn ask_ai_addr_streaming<F>(server_addr: SocketAddr, prompt: &str, on_token: F) -> Result<String, String>
where
    F: FnMut(&str) + Send,
{
    ask_ai_addr_streaming_with_sni(server_addr, server_addr.ip().to_string(), prompt, on_token)
}

fn ask_ai_addr_streaming_with_sni<F>(
    server_addr: SocketAddr,
    server_name: String,
    prompt: &str,
    on_token: F,
) -> Result<String, String>
where
    F: FnMut(&str) + Send,
{
    ask_ai_addr_streaming_with_sni_and_rich(server_addr, server_name, prompt, on_token, |_| {})
}

fn ask_ai_addr_streaming_with_sni_and_rich<F, G>(
    server_addr: SocketAddr,
    server_name: String,
    prompt: &str,
    on_token: F,
    on_rich: G,
) -> Result<String, String>
where
    F: FnMut(&str) + Send,
    G: FnMut(&str) + Send,
{
    ask_ai_addr_streaming_full(server_addr, server_name, prompt, on_token, on_rich, |_| {}, |_| {}, |_| {}, |_| {})
}

fn ask_ai_addr_streaming_full<F, G, H, I, J, K>(
    server_addr: SocketAddr,
    server_name: String,
    prompt: &str,
    on_token: F,
    on_rich: G,
    on_style: H,
    on_layout: I,
    on_audio: J,
    on_control: K,
) -> Result<String, String>
where
    F: FnMut(&str) + Send,
    G: FnMut(&str) + Send,
    H: FnMut(&str) + Send,
    I: FnMut(&str) + Send,
    J: FnMut(&str) + Send,
    K: FnMut(&str) + Send,
{
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    rt.block_on(async_run_streaming(
        server_addr, &server_name, prompt, provider,
        on_token, on_rich, on_style, on_layout, on_audio, on_control,
    )).map_err(|e| e.to_string())
}

async fn async_run_streaming<F, G, H, I, J, K>(
    server_addr: SocketAddr,
    server_name: &str,
    prompt: &str,
    provider: Arc<CryptoProvider>,
    on_token: F,
    on_rich: G,
    on_style: H,
    on_layout: I,
    on_audio: J,
    on_control: K,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>>
where
    F: FnMut(&str) + Send,
    G: FnMut(&str) + Send,
    H: FnMut(&str) + Send,
    I: FnMut(&str) + Send,
    J: FnMut(&str) + Send,
    K: FnMut(&str) + Send,
{
    let client_config = configure_client(provider)?;
    let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
    endpoint.set_default_client_config(client_config);

    const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
    let connection = match tokio::time::timeout(
        TIMEOUT,
        endpoint.connect(server_addr, server_name)?,
    )
    .await
    {
        Ok(Ok(conn)) => conn,
        Ok(Err(e)) => {
            let msg = format!("{e}");
            let friendly = if msg.to_lowercase().contains("connection refused")
                || msg.to_lowercase().contains("unreachable")
            {
                "Couldn't reach the server. Check the address and that you're on the same network."
            } else {
                "Couldn't connect. Check the server address and try again."
            };
            return Err(std::io::Error::new(std::io::ErrorKind::Other, friendly).into());
        }
        Err(_) => return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "The server took too long to respond. Check the address and try again.",
        ).into()),
    };

    let (mut send, mut recv) = connection.open_bi().await?;
    send.write_all(prompt.as_bytes()).await?;
    send.finish()?;

    let result = parse_stream_response::<4096, _, _, _, _, _, _>(
        &mut recv, TIMEOUT,
        on_token, on_rich, on_style, on_layout, on_audio, on_control,
    ).await?;

    connection.close(0u32.into(), b"done");
    endpoint.wait_idle().await;

    Ok(result)
}

/// Parse newline-delimited JSON streaming frames from the ember server.
/// Calls callbacks for each frame type: token (chat), rich (HTML), style (CSS), layout (JSON), audio (TTS), control_payload.
async fn parse_stream_response<const BUF: usize, F, G, H, I, J, K>(
    recv: &mut quinn::RecvStream,
    timeout: std::time::Duration,
    mut on_token: F,
    mut on_rich: G,
    mut on_style: H,
    mut on_layout: I,
    mut on_audio: J,
    mut on_control: K,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>>
where
    F: FnMut(&str),
    G: FnMut(&str),
    H: FnMut(&str),
    I: FnMut(&str),
    J: FnMut(&str),
    K: FnMut(&str),
{
    let mut buf = Vec::with_capacity(4096);
    let mut result = String::new();
    let mut legacy_mode = false;

    loop {
        let mut chunk = vec![0u8; BUF];
        let n = match tokio::time::timeout(timeout, recv.read(&mut chunk)).await {
            Ok(Ok(Some(n))) => n,
            Ok(Ok(None)) => break,
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "The response took too long. Try again in a moment.",
            ).into()),
        };
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);

        // If we already switched to legacy mode, keep accumulating
        if legacy_mode {
            continue;
        }

        // Process complete lines
        while let Some(idx) = buf.iter().position(|&b| b == b'\n') {
            let line = std::mem::take(&mut buf);
            let (complete, rest) = line.split_at(idx);
            buf = rest[1..].to_vec(); // skip the \n

            let line_str = String::from_utf8_lossy(complete);
            let line_str = line_str.trim();
            if line_str.is_empty() {
                continue;
            }

            // Try JSON first; if it fails and line doesn't look like JSON, treat as legacy
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line_str) {
                let msg_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match msg_type {
                    "stream_token" => {
                        if let Some(token) = v.get("token").and_then(|t| t.as_str()) {
                            result.push_str(token);
                            on_token(token);
                        }
                    }
                    "stream_rich" => {
                        if let Some(content) = v.get("content").and_then(|c| c.as_str()) {
                            on_rich(content);
                        }
                    }
                    "stream_style" => {
                        if let Some(css) = v.get("css").and_then(|c| c.as_str()) {
                            on_style(css);
                        }
                    }
                    "stream_layout" => {
                        if let Some(json) = v.get("json").and_then(|j| j.as_str()) {
                            on_layout(json);
                        }
                    }
                    "stream_audio" => {
                        if let Some(text) = v.get("text").and_then(|t| t.as_str()) {
                            on_audio(text);
                        }
                    }
                    "stream_control_payload" => {
                        if let Some(payload) = v.get("payload").and_then(|p| p.as_str()) {
                            on_control(payload);
                        }
                    }
                    "stream_end" => {
                        return Ok(result);
                    }
                    "stream_error" => {
                        let err = v.get("error").and_then(|e| e.as_str()).unwrap_or("Something went wrong. Try again.");
                        return Err(format!("Error: {}", err).into());
                    }
                    "stream_start" => {}
                    _ => {}
                }
            } else if !line_str.starts_with('{') {
                // Legacy format: raw text, not JSON. Accumulate and read rest of stream.
                legacy_mode = true;
                result.push_str(line_str);
                result.push('\n');
                on_token(line_str);
                on_token("\n");
                break;
            } else {
                return Err("Something went wrong with the response. Try again.".into());
            }
        }
    }

    // Add any remaining buffer: legacy continuation or entire response with no newlines
    if !buf.is_empty() {
        let rest = String::from_utf8_lossy(&buf);
        result.push_str(&rest);
        on_token(&rest);
    }

    Ok(result)
}
