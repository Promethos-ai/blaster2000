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
pub fn ask_ai(server_addr: impl ToSocketAddrs, prompt: &str) -> Result<String, String> {
    let addrs: Vec<SocketAddr> = server_addr.to_socket_addrs().map_err(|e| e.to_string())?.collect();
    let server_addr = addrs
        .iter()
        .find(|a| a.is_ipv4())
        .copied()
        .or_else(|| addrs.into_iter().next())
        .ok_or_else(|| "Could not resolve address".to_string())?;
    ask_ai_addr(server_addr, prompt)
}

/// Ask the AI (with pre-resolved address).
pub fn ask_ai_addr(server_addr: SocketAddr, prompt: &str) -> Result<String, String> {
    ask_ai_addr_streaming(server_addr, prompt, |_| {})
}

/// Ask the AI with a callback for each token (for progressive UI display).
pub fn ask_ai_addr_streaming<F>(server_addr: SocketAddr, prompt: &str, on_token: F) -> Result<String, String>
where
    F: FnMut(&str) + Send,
{
    let provider = Arc::new(rustls::crypto::ring::default_provider());

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;

    rt.block_on(async_run_streaming(server_addr, prompt, provider, on_token)).map_err(|e| e.to_string())
}

async fn async_run_streaming<F>(
    server_addr: SocketAddr,
    prompt: &str,
    provider: Arc<CryptoProvider>,
    on_token: F,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>>
where
    F: FnMut(&str) + Send,
{
    let client_config = configure_client(provider)?;
    let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
    endpoint.set_default_client_config(client_config);

    const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);
    let server_name = server_addr.ip().to_string();
    let connection = match tokio::time::timeout(
        TIMEOUT,
        endpoint.connect(server_addr, &server_name)?,
    )
    .await
    {
        Ok(Ok(conn)) => conn,
        Ok(Err(e)) => return Err(format!("connection failed: {e}").into()),
        Err(_) => return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "connection timeout (check server address and network)",
        ).into()),
    };

    let (mut send, mut recv) = connection.open_bi().await?;
    send.write_all(prompt.as_bytes()).await?;
    send.finish()?;

    let result = parse_stream_response::<64>(&mut recv, TIMEOUT, on_token).await?;

    connection.close(0u32.into(), b"done");
    endpoint.wait_idle().await;

    Ok(result)
}

/// Parse newline-delimited JSON streaming frames from the ember server.
/// Calls on_token for each stream_token (for progressive UI updates).
/// Falls back to raw text if server sends legacy (pre-streaming) format.
async fn parse_stream_response<const BUF: usize>(
    recv: &mut quinn::RecvStream,
    timeout: std::time::Duration,
    mut on_token: impl FnMut(&str),
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
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
                "response timeout",
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
                    "stream_end" => {
                        return Ok(result);
                    }
                    "stream_error" => {
                        let err = v.get("error").and_then(|e| e.as_str()).unwrap_or("unknown error");
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
                return Err(format!("invalid JSON: expected value at line 1 column 1").into());
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
