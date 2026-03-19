//! # bolt402-mcp
//!
//! MCP server binary that exposes bolt402's L402 capabilities as MCP tools.
//!
//! Enables any MCP-compatible AI agent (Claude Code, Cursor, Codex, `OpenClaw`)
//! to make L402-gated API requests without framework-specific integration.
//!
//! ## Usage
//!
//! ```bash
//! # Mock backend (testing)
//! bolt402-mcp --backend mock
//!
//! # LND backend
//! bolt402-mcp --backend lnd \
//!   --lnd-url https://localhost:10009 \
//!   --lnd-macaroon /path/to/admin.macaroon \
//!   --lnd-cert /path/to/tls.cert
//! ```

mod cli;
mod server;

use std::collections::HashMap;

use clap::Parser;
use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

use bolt402_core::budget::Budget;
use bolt402_core::cache::InMemoryTokenStore;
use bolt402_core::{L402Client, L402ClientConfig};

use crate::cli::{Backend, Cli};
use crate::server::Bolt402McpServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging (stderr, to keep stdout clean for MCP stdio transport).
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("bolt402_mcp=info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    let budget = Budget {
        per_request_max: cli.budget_per_request,
        hourly_max: cli.budget_hourly,
        daily_max: cli.budget_daily,
        total_max: cli.budget_total,
        domain_budgets: HashMap::new(),
    };

    let config = L402ClientConfig {
        max_fee_sats: cli.max_fee_sats,
        ..L402ClientConfig::default()
    };

    let client = match cli.backend {
        Backend::Lnd => {
            let url = cli
                .lnd_url
                .ok_or("--lnd-url is required when --backend=lnd")?;
            let macaroon = cli
                .lnd_macaroon
                .ok_or("--lnd-macaroon is required when --backend=lnd")?;
            let cert = cli
                .lnd_cert
                .ok_or("--lnd-cert is required when --backend=lnd")?;

            let backend = bolt402_lnd::LndBackend::connect(&url, &cert, &macaroon).await?;

            L402Client::builder()
                .ln_backend(backend)
                .token_store(InMemoryTokenStore::default())
                .budget(budget.clone())
                .config(config)
                .build()?
        }
        Backend::Mock => {
            // Create a mock server for testing. The mock server provides
            // both an HTTP endpoint and a mock Lightning backend.
            let mock_server = bolt402_mock::MockL402Server::builder()
                .endpoint("/api/data", bolt402_mock::EndpointConfig::new(100))
                .build()
                .await?;

            let backend = mock_server.mock_backend();

            tracing::info!(url = %mock_server.url(), "mock L402 server started");

            // Leak the server so it stays alive for the process lifetime.
            // This is fine for a long-running server binary.
            Box::leak(Box::new(mock_server));

            L402Client::builder()
                .ln_backend(backend)
                .token_store(InMemoryTokenStore::default())
                .budget(budget.clone())
                .config(config)
                .build()?
        }
    };

    let server = Bolt402McpServer::new(client, budget);

    tracing::info!("bolt402 MCP server starting on stdio");

    let transport = rmcp::transport::stdio();
    let service = server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}
