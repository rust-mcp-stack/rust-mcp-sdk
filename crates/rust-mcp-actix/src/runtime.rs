use crate::server::ActixServer;
use actix_web::dev::ServerHandle;
use rust_mcp_sdk::SessionId;
use rust_mcp_sdk::{error::SdkResult, mcp_http::McpAppState};
use std::io;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

/// Runtime handle for a running Actix MCP server.
///
/// Provides session management, graceful shutdown, and per-session request/notification
/// methods. Implements [`McpHttpServer`] for framework-agnostic usage.
pub struct ActixRuntime {
    pub(crate) state: Arc<McpAppState>,
    pub(crate) server_task: JoinHandle<io::Result<()>>,
    pub(crate) server_handle: ServerHandle,
}

impl ActixRuntime {
    /// Creates and starts a new runtime from an `ActixServer`.
    pub async fn create(server: ActixServer) -> SdkResult<Self> {
        let addr = server
            .options()
            .resolve_server_address()
            .map_err(|e| rust_mcp_sdk::error::McpSdkError::Internal { description: e })?;

        let state = server.state();
        let info = server.server_info(Some(addr)).unwrap_or_default();
        tracing::info!("{}", info);

        let state_clone = state.clone();
        let handler = server.handler.clone();
        let mount_options = server.options().resolve_mount_options();

        let srv = actix_web::HttpServer::new(move || {
            actix_web::App::new().service(crate::mcp_scope(
                state_clone.clone(),
                handler.clone(),
                &mount_options,
            ))
        });

        #[cfg(feature = "ssl")]
        let srv = if server.options().enable_ssl {
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
            let config = load_rustls_config(
                server
                    .options()
                    .ssl_cert_path
                    .as_deref()
                    .unwrap_or_default(),
                server.options().ssl_key_path.as_deref().unwrap_or_default(),
            )
            .map_err(|e| rust_mcp_sdk::error::McpSdkError::Internal {
                description: e.to_string(),
            })?;
            srv.bind_rustls_0_23(addr, config).map_err(|e| {
                rust_mcp_sdk::error::McpSdkError::Internal {
                    description: e.to_string(),
                }
            })?
        } else {
            srv.bind(addr)
                .map_err(|e| rust_mcp_sdk::error::McpSdkError::Internal {
                    description: e.to_string(),
                })?
        };

        #[cfg(not(feature = "ssl"))]
        let srv = srv
            .bind(addr)
            .map_err(|e| rust_mcp_sdk::error::McpSdkError::Internal {
                description: e.to_string(),
            })?;

        let srv = srv.run();

        let server_handle = srv.handle();
        let server_task = tokio::spawn(srv);

        Ok(Self {
            state,
            server_task,
            server_handle,
        })
    }

    /// Gracefully stops the server.
    pub fn graceful_shutdown(&self, _timeout: Option<Duration>) {
        let handle = self.server_handle.clone();
        let state = self.state.clone();
        tokio::spawn(async move {
            // Close long-lived `subscriptions/listen` streams so the server
            // drains without hanging on open SSE bodies.
            state.shutdown_all_listen_streams().await;
            let _ = handle.stop(true).await;
        });
    }

    /// Awaits server completion (typically until shutdown).
    pub async fn await_server(self) -> SdkResult<()> {
        self.server_task
            .await
            .map_err(|e| rust_mcp_sdk::error::McpSdkError::Internal {
                description: e.to_string(),
            })?
            .map_err(|e| rust_mcp_sdk::error::McpSdkError::Internal {
                description: e.to_string(),
            })
    }

    /// Returns all active session IDs.
    pub async fn sessions(&self) -> Vec<String> {
        Vec::new()
    }

    pub async fn runtime_by_session(
        &self,
        session_id: &SessionId,
    ) -> Result<Arc<ServerRuntime>, rust_mcp_sdk::error::McpSdkError> {
        Err(rust_mcp_sdk::error::McpSdkError::Internal {
            description: format!("Session not found: {}", session_id),
        })
    }
}

use async_trait::async_trait;
use rust_mcp_sdk::mcp_server::ServerRuntime;
use rust_mcp_sdk::McpHttpServer;

#[async_trait]
impl McpHttpServer for ActixRuntime {
    async fn graceful_shutdown(&self) {
        self.graceful_shutdown(None);
    }

    async fn sessions(&self) -> Vec<SessionId> {
        ActixRuntime::sessions(self).await
    }

    async fn runtime_by_session(&self, id: &SessionId) -> SdkResult<Arc<ServerRuntime>> {
        ActixRuntime::runtime_by_session(self, id).await
    }
}

#[cfg(feature = "ssl")]
fn load_rustls_config(cert_path: &str, key_path: &str) -> std::io::Result<rustls::ServerConfig> {
    use rustls::pki_types::pem::{Error as PemError, PemObject};
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    use std::io::{Error, ErrorKind};

    // Preserve raw io::Error for file I/O failures (e.g. NotFound);
    // map PEM parse failures to InvalidData.
    let map_err = |e: PemError| match e {
        PemError::Io(io) => io,
        other => Error::new(ErrorKind::InvalidData, other),
    };

    // Fail loud on any malformed cert section — never silently drop chain entries.
    let certs = CertificateDer::pem_file_iter(cert_path)
        .map_err(map_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_err)?;

    let key = PrivateKeyDer::from_pem_file(key_path).map_err(map_err)?;

    rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| Error::new(ErrorKind::InvalidInput, e))
}

#[cfg(all(test, feature = "ssl"))]
mod ssl_tests {
    #[test]
    fn install_crypto_provider_idempotent() {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    }

    #[test]
    fn load_rustls_config_parses_pem_cert_and_key() {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        // Generate test cert + key on the fly
        use std::process::Command;
        let cert_path = "/tmp/test_actix_cert.pem";
        let key_path = "/tmp/test_actix_key.pem";

        let keygen = Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-keyout",
                key_path,
                "-out",
                cert_path,
                "-days",
                "1",
                "-nodes",
                "-subj",
                "/CN=localhost",
            ])
            .output()
            .expect("openssl must be installed");
        assert!(keygen.status.success(), "openssl keygen failed");

        let result = super::load_rustls_config(cert_path, key_path);
        assert!(
            result.is_ok(),
            "load_rustls_config failed: {:?}",
            result.err()
        );
    }

    #[test]
    fn load_rustls_config_fails_loud_on_missing_key() {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

        use std::process::Command;
        let cert_path = "/tmp/test_actix_cert_nokey.pem";
        let key_path = "/tmp/test_actix_key_nokey.pem";

        let keygen = Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-keyout",
                key_path,
                "-out",
                cert_path,
                "-days",
                "1",
                "-nodes",
                "-subj",
                "/CN=localhost",
            ])
            .output()
            .expect("openssl must be installed");
        assert!(keygen.status.success(), "openssl keygen failed");

        // Point key_path at the cert file: PEM sections exist but hold no private key.
        let result = super::load_rustls_config(cert_path, cert_path);
        assert!(
            result.is_err(),
            "expected error when key file has no private key"
        );
    }
}
