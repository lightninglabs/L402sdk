//! CLI argument parsing for the bolt402 MCP server.

use clap::{Parser, ValueEnum};

/// bolt402 MCP server — expose L402 payment tools to any MCP-compatible AI agent.
#[derive(Debug, Parser)]
#[command(name = "bolt402-mcp", version, about)]
pub(crate) struct Cli {
    /// Lightning backend to use.
    #[arg(long, default_value = "mock", env = "BOLT402_BACKEND")]
    pub backend: Backend,

    /// LND gRPC endpoint URL (required for --backend=lnd).
    #[arg(long, env = "BOLT402_LND_URL")]
    pub lnd_url: Option<String>,

    /// Path to LND admin macaroon file (required for --backend=lnd).
    #[arg(long, env = "BOLT402_LND_MACAROON")]
    pub lnd_macaroon: Option<String>,

    /// Path to LND TLS certificate file (required for --backend=lnd).
    #[arg(long, env = "BOLT402_LND_CERT")]
    pub lnd_cert: Option<String>,

    /// Maximum satoshis per individual payment.
    #[arg(long, env = "BOLT402_BUDGET_PER_REQUEST")]
    pub budget_per_request: Option<u64>,

    /// Maximum satoshis per hour.
    #[arg(long, env = "BOLT402_BUDGET_HOURLY")]
    pub budget_hourly: Option<u64>,

    /// Maximum satoshis per day.
    #[arg(long, env = "BOLT402_BUDGET_DAILY")]
    pub budget_daily: Option<u64>,

    /// Maximum total satoshis to spend.
    #[arg(long, env = "BOLT402_BUDGET_TOTAL")]
    pub budget_total: Option<u64>,

    /// Maximum routing fee in satoshis when paying invoices.
    #[arg(long, default_value = "100", env = "BOLT402_MAX_FEE_SATS")]
    pub max_fee_sats: u64,
}

/// Available Lightning backends.
#[derive(Debug, Clone, ValueEnum)]
pub(crate) enum Backend {
    /// LND via gRPC (requires --lnd-url, --lnd-macaroon, --lnd-cert).
    Lnd,
    /// Mock backend for testing (no real payments).
    Mock,
}
