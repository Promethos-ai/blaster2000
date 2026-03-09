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
    let provider = Arc::new(rustls::crypto::ring::default_provider());

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;

    rt.block_on(async_run(server_addr, prompt, provider)).map_err(|e| e.to_string())
}

async fn async_run(server_addr: SocketAddr, prompt: &str, provider: Arc<CryptoProvider>) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

    let client_config = configure_client(provider)?;
    let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
    endpoint.set_default_client_config(client_config);

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

    let response = match tokio::time::timeout(TIMEOUT, recv.read_to_end(64 * 1024)).await {
        Ok(Ok(data)) => data,
        Ok(Err(e)) => return Err(e.into()),
        Err(_) => return Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "response timeout (server may be slow or unreachable)",
        ).into()),
    };
    let result = String::from_utf8_lossy(&response).to_string();

    connection.close(0u32.into(), b"done");
    endpoint.wait_idle().await;

    Ok(result)
}
