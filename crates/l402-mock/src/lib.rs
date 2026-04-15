//! # l402-mock
//!
//! Mock L402 server and Lightning backend for testing L402sdk clients.
//!
//! Provides two key components:
//!
//! - [`MockL402Server`]: An HTTP server that implements L402 authentication,
//!   returning 402 challenges and validating payment tokens.
//! - [`MockLnBackend`]: A fake Lightning backend that "pays" invoices by
//!   looking up preimages from the mock server's registry.
//!
//! Together, these let you test the full L402 flow without any real Lightning
//! infrastructure.
//!
//! ## Example
//!
//! ```rust,no_run
//! use l402_mock::{MockL402Server, EndpointConfig};
//!
//! # async fn example() {
//! let server = MockL402Server::builder()
//!     .endpoint("/api/data", EndpointConfig::new(100))
//!     .build()
//!     .await
//!     .unwrap();
//!
//! let backend = server.mock_backend();
//! println!("Mock server running at {}", server.url());
//! # }
//! ```

mod backend;
mod challenge;
mod server;

pub use backend::MockLnBackend;
pub use challenge::PendingChallenge;
pub use server::{EndpointConfig, MockL402Server, MockL402ServerBuilder};
