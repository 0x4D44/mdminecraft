//! QUIC transport layer using quinn for reliable, encrypted connections.
//!
//! Provides server and client endpoints with TLS encryption (self-signed certs for development).

use anyhow::{Context, Result};
use quinn::{ClientConfig, Endpoint, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{debug, info};

/// Server endpoint for accepting QUIC connections.
pub struct ServerEndpoint {
    endpoint: Endpoint,
    addr: SocketAddr,
}

impl ServerEndpoint {
    /// Create a new server endpoint bound to the given address.
    ///
    /// Uses self-signed TLS certificates for development.
    pub fn bind(addr: SocketAddr) -> Result<Self> {
        info!("Creating server endpoint on {}", addr);

        // Install default crypto provider if not already installed
        let _ = rustls::crypto::ring::default_provider().install_default();

        // Generate self-signed certificate for development
        let (cert, key) = generate_self_signed_cert()?;

        // Configure rustls with the certificate
        let mut server_crypto = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert.clone()], key.clone_key())
            .context("Failed to build rustls ServerConfig")?;

        server_crypto.alpn_protocols = vec![b"mdminecraft".to_vec()];

        // Configure quinn server
        let mut server_config = ServerConfig::with_crypto(Arc::new(
            quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto)
                .context("Failed to create QuicServerConfig")?,
        ));

        // Configure transport parameters
        let mut transport_config = quinn::TransportConfig::default();
        transport_config.keep_alive_interval(Some(std::time::Duration::from_secs(5)));
        transport_config.max_idle_timeout(Some(std::time::Duration::from_secs(30).try_into()?));

        server_config.transport_config(Arc::new(transport_config));

        // Bind the endpoint
        let endpoint = Endpoint::server(server_config, addr)
            .context("Failed to bind server endpoint")?;

        let actual_addr = endpoint.local_addr()?;
        info!("Server endpoint bound to {}", actual_addr);

        Ok(Self {
            endpoint,
            addr: actual_addr,
        })
    }

    /// Get the local address this endpoint is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Accept an incoming connection.
    ///
    /// Returns None when the endpoint is closed.
    pub async fn accept(&self) -> Option<quinn::Incoming> {
        self.endpoint.accept().await
    }

    /// Close the endpoint, rejecting new connections.
    pub fn close(&self) {
        self.endpoint.close(0u32.into(), b"Server shutting down");
    }
}

/// Client endpoint for establishing QUIC connections.
pub struct ClientEndpoint {
    endpoint: Endpoint,
}

impl ClientEndpoint {
    /// Create a new client endpoint.
    ///
    /// Accepts self-signed certificates for development (insecure).
    pub fn new() -> Result<Self> {
        debug!("Creating client endpoint");

        // Install default crypto provider if not already installed
        let _ = rustls::crypto::ring::default_provider().install_default();

        // Configure rustls to accept self-signed certificates (development only)
        let mut client_crypto = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
            .with_no_client_auth();

        client_crypto.alpn_protocols = vec![b"mdminecraft".to_vec()];

        // Configure quinn client
        let client_config = ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(client_crypto)
                .context("Failed to create QuicClientConfig")?,
        ));

        // Bind to any available port
        let mut endpoint = Endpoint::client("0.0.0.0:0".parse()?)?;
        endpoint.set_default_client_config(client_config);

        debug!("Client endpoint created on {}", endpoint.local_addr()?);

        Ok(Self { endpoint })
    }

    /// Connect to a server at the given address.
    ///
    /// Returns the established connection.
    pub async fn connect(&self, server_addr: SocketAddr) -> Result<quinn::Connection> {
        info!("Connecting to server at {}", server_addr);

        let connection = self
            .endpoint
            .connect(server_addr, "localhost")
            .context("Failed to initiate connection")?
            .await
            .context("Failed to establish connection")?;

        info!("Connected to server at {}", server_addr);

        Ok(connection)
    }

    /// Close the endpoint, terminating all connections.
    pub fn close(&self) {
        self.endpoint.close(0u32.into(), b"Client shutting down");
    }
}

impl Default for ClientEndpoint {
    fn default() -> Self {
        Self::new().expect("Failed to create default client endpoint")
    }
}

/// Generate a self-signed certificate for development use.
///
/// **WARNING:** This is insecure and should only be used for development/testing.
fn generate_self_signed_cert() -> Result<(CertificateDer<'static>, PrivateKeyDer<'static>)> {
    debug!("Generating self-signed certificate");

    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
        .context("Failed to generate certificate")?;

    let key = PrivateKeyDer::Pkcs8(cert.key_pair.serialize_der().into());
    let cert_der = CertificateDer::from(cert.cert);

    Ok((cert_der, key))
}

/// Certificate verifier that accepts all certificates (development only).
///
/// **WARNING:** This bypasses TLS security and should NEVER be used in production.
#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ED25519,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_bind() {
        let server = ServerEndpoint::bind("127.0.0.1:0".parse().unwrap())
            .expect("Failed to bind server");
        assert!(server.local_addr().port() > 0);
    }

    #[tokio::test]
    async fn test_client_creation() {
        let client = ClientEndpoint::new().expect("Failed to create client");
        client.close();
    }

    #[tokio::test]
    async fn test_connection_handshake() {
        // Start server
        let server = ServerEndpoint::bind("127.0.0.1:0".parse().unwrap())
            .expect("Failed to bind server");
        let server_addr = server.local_addr();

        // Spawn server accept task
        let server_handle = tokio::spawn(async move {
            if let Some(incoming) = server.accept().await {
                incoming.await.expect("Failed to accept connection")
            } else {
                panic!("Server closed before accepting connection")
            }
        });

        // Connect client
        let client = ClientEndpoint::new().expect("Failed to create client");
        let client_conn = client
            .connect(server_addr)
            .await
            .expect("Failed to connect");

        // Wait for server to accept
        let server_conn = server_handle.await.expect("Server task panicked");

        // Verify connections are established
        assert_eq!(client_conn.remote_address(), server_addr);
        assert!(server_conn.remote_address().port() > 0);

        // Cleanup
        client_conn.close(0u32.into(), b"Test complete");
        server_conn.close(0u32.into(), b"Test complete");
    }
}
