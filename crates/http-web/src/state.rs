//! Server state and the public [`Server`] entrypoint.

use crate::router;
use http_core::dispatch::Dispatcher;
use std::net::SocketAddr;
use std::sync::Arc;

/// Shared application state available to every axum handler via
/// [`axum::extract::State`]. Cloning is cheap — the inner `Dispatcher`
/// is wrapped in an `Arc` (it already wraps a `reqwest::Client`, which
/// itself is cheap to clone).
#[derive(Clone)]
pub struct AppState {
    /// HTTP dispatcher (reqwest-backed). One is shared by all requests —
    /// reqwest manages its own connection pool internally.
    pub dispatcher: Arc<Dispatcher>,
    /// Optional bearer token. When `Some`, every request must carry
    /// `Authorization: Bearer <token>` (case-sensitive). When `None`,
    /// no auth is enforced.
    pub auth_token: Option<String>,
}

impl AppState {
    pub fn new(dispatcher: Dispatcher, auth_token: Option<String>) -> Self {
        Self {
            dispatcher: Arc::new(dispatcher),
            auth_token,
        }
    }
}

/// Builder for an [`AppState`] + the configured axum router.
///
/// ```
/// # use http_web::Server;
/// # async fn demo() {
/// let server = Server::builder()
///     .with_token("s3cret".to_string())
///     .build();
/// # }
/// ```
pub struct ServerBuilder {
    dispatcher: Dispatcher,
    auth_token: Option<String>,
}

impl Default for ServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerBuilder {
    pub fn new() -> Self {
        Self {
            dispatcher: Dispatcher::new(),
            auth_token: None,
        }
    }

    /// Use a pre-configured dispatcher (custom TLS / timeout / proxy).
    pub fn with_dispatcher(mut self, d: Dispatcher) -> Self {
        self.dispatcher = d;
        self
    }

    /// Require `Authorization: Bearer <token>` on every route.
    pub fn with_token(mut self, token: String) -> Self {
        self.auth_token = Some(token);
        self
    }

    pub fn build(self) -> Server {
        let state = AppState::new(self.dispatcher, self.auth_token);
        let app = router(state);
        Server { app }
    }
}

/// A ready-to-serve axum app plus the original builder config.
///
/// Use [`Server::bind_ephemeral`] in tests to grab a random port.
pub struct Server {
    pub app: axum::Router,
}

impl Server {
    /// Convenience constructor — equivalent to `ServerBuilder::new().build()`.
    pub fn builder() -> ServerBuilder {
        ServerBuilder::new()
    }

    /// Bind to an ephemeral localhost port and return the bound address.
    /// Caller is responsible for spawning the server task; tests typically
    /// spawn `tokio::spawn(async move { serve().await })` immediately.
    pub async fn bind_ephemeral(&self) -> SocketAddr {
        use tokio::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral");
        let addr = listener.local_addr().expect("local_addr");
        let app = self.app.clone();
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("axum serve");
        });
        addr
    }

    /// Bind to a specific address (e.g. `0.0.0.0:8080`) and serve
    /// forever. Intended for the production binary (Docker / direct
    /// invocation). Logs go to stderr.
    pub async fn serve_addr(self, addr: &str) -> std::io::Result<()> {
        use tokio::net::TcpListener;
        let listener = TcpListener::bind(addr).await?;
        tracing::info!("http-web listening on {addr}");
        axum::serve(listener, self.app).await
    }
}
