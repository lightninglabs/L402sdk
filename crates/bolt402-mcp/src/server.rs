//! MCP server implementation for bolt402.
//!
//! Wraps the [`L402Client`] and exposes its capabilities as MCP tools:
//! - `l402_request`: Make HTTP requests through L402-gated APIs
//! - `check_budget`: Query spending statistics
//! - `list_receipts`: Retrieve payment receipts for audit

use std::sync::Arc;

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::router::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};

use bolt402_core::L402Client;
use bolt402_core::budget::Budget;

/// MCP tool input for `l402_request`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct L402RequestParams {
    /// The URL to request.
    pub url: String,

    /// HTTP method (GET, POST, PUT, DELETE). Defaults to GET.
    #[serde(default = "default_method")]
    pub method: String,

    /// Optional JSON request body (for POST/PUT).
    #[serde(default)]
    pub body: Option<String>,
}

fn default_method() -> String {
    "GET".to_string()
}

/// MCP tool input for `list_receipts`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ListReceiptsParams {
    /// Maximum number of receipts to return.
    #[serde(default)]
    pub limit: Option<usize>,

    /// Filter receipts by endpoint substring.
    #[serde(default)]
    pub endpoint_filter: Option<String>,
}

/// Response from `l402_request` tool.
#[derive(Debug, Serialize)]
struct L402RequestResponse {
    status: u16,
    body: String,
    paid: bool,
    receipt: Option<ReceiptResponse>,
}

/// Serializable receipt for MCP tool output.
#[derive(Debug, Serialize)]
struct ReceiptResponse {
    timestamp: u64,
    endpoint: String,
    amount_sats: u64,
    fee_sats: u64,
    total_cost_sats: u64,
    payment_hash: String,
    response_status: u16,
    latency_ms: u64,
}

/// Budget status response for `check_budget` tool.
#[derive(Debug, Serialize)]
struct BudgetStatusResponse {
    total_spent_sats: u64,
    budget: BudgetConfigResponse,
}

/// Budget configuration for reporting.
#[derive(Debug, Serialize)]
#[allow(clippy::struct_field_names)]
struct BudgetConfigResponse {
    per_request_max: Option<u64>,
    hourly_max: Option<u64>,
    daily_max: Option<u64>,
    total_max: Option<u64>,
}

impl From<&Budget> for BudgetConfigResponse {
    fn from(budget: &Budget) -> Self {
        Self {
            per_request_max: budget.per_request_max,
            hourly_max: budget.hourly_max,
            daily_max: budget.daily_max,
            total_max: budget.total_max,
        }
    }
}

/// bolt402 MCP server that exposes L402 payment tools.
#[derive(Clone)]
pub(crate) struct Bolt402McpServer {
    client: Arc<L402Client>,
    budget: Budget,
    tool_router: ToolRouter<Self>,
}

impl Bolt402McpServer {
    /// Create a new MCP server wrapping the given L402 client and budget config.
    pub(crate) fn new(client: L402Client, budget: Budget) -> Self {
        Self {
            client: Arc::new(client),
            budget,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl Bolt402McpServer {
    /// Make an HTTP request to an L402-gated API. Automatically handles the
    /// full L402 flow: detects 402 challenges, pays Lightning invoices, and
    /// retries with valid credentials.
    #[tool(
        name = "l402_request",
        description = "Make an HTTP request to an L402-gated API. Handles payment challenges automatically: if the server responds with HTTP 402, pays the Lightning invoice and retries with credentials."
    )]
    async fn l402_request(
        &self,
        Parameters(params): Parameters<L402RequestParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = match params.method.to_uppercase().as_str() {
            "GET" => self.client.get(&params.url).await,
            "POST" => self.client.post(&params.url, params.body.as_deref()).await,
            other => {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Unsupported HTTP method: {other}. Supported: GET, POST"
                ))]));
            }
        };

        match result {
            Ok(response) => {
                let status = response.status().as_u16();
                let paid = response.paid();
                let receipt = response.receipt().map(|r| ReceiptResponse {
                    timestamp: r.timestamp,
                    endpoint: r.endpoint.clone(),
                    amount_sats: r.amount_sats,
                    fee_sats: r.fee_sats,
                    total_cost_sats: r.total_cost_sats(),
                    payment_hash: r.payment_hash.clone(),
                    response_status: r.response_status,
                    latency_ms: r.latency_ms,
                });

                let body = response.text().await.unwrap_or_default();

                let resp = L402RequestResponse {
                    status,
                    body,
                    paid,
                    receipt,
                };

                let json =
                    serde_json::to_string_pretty(&resp).unwrap_or_else(|e| format!("Error: {e}"));
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                "L402 request failed: {e}"
            ))])),
        }
    }

    /// Query the current budget status including total spending and configured limits.
    #[tool(
        name = "check_budget",
        description = "Check the current budget status: total amount spent and configured spending limits (per-request, hourly, daily, total)."
    )]
    async fn check_budget(&self) -> Result<CallToolResult, McpError> {
        let total_spent = self.client.total_spent().await;

        let resp = BudgetStatusResponse {
            total_spent_sats: total_spent,
            budget: BudgetConfigResponse::from(&self.budget),
        };

        let json = serde_json::to_string_pretty(&resp).unwrap_or_else(|e| format!("Error: {e}"));
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    /// Get payment receipts for all L402 payments made during this session.
    #[tool(
        name = "list_receipts",
        description = "List payment receipts for all L402 payments made in this session. Supports filtering by endpoint and limiting the number of results."
    )]
    async fn list_receipts(
        &self,
        Parameters(params): Parameters<ListReceiptsParams>,
    ) -> Result<CallToolResult, McpError> {
        let receipts = self.client.receipts().await;

        let filtered: Vec<ReceiptResponse> = receipts
            .into_iter()
            .filter(|r| {
                params
                    .endpoint_filter
                    .as_ref()
                    .is_none_or(|f| r.endpoint.contains(f.as_str()))
            })
            .take(params.limit.unwrap_or(usize::MAX))
            .map(|r| {
                let total = r.total_cost_sats();
                ReceiptResponse {
                    timestamp: r.timestamp,
                    endpoint: r.endpoint,
                    amount_sats: r.amount_sats,
                    fee_sats: r.fee_sats,
                    total_cost_sats: total,
                    payment_hash: r.payment_hash,
                    response_status: r.response_status,
                    latency_ms: r.latency_ms,
                }
            })
            .collect();

        let json =
            serde_json::to_string_pretty(&filtered).unwrap_or_else(|e| format!("Error: {e}"));
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

#[tool_handler]
impl ServerHandler for Bolt402McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "bolt402-mcp",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "bolt402 MCP server: Make HTTP requests to L402 (Lightning-gated) APIs. \
                 The server automatically handles payment challenges — when an API responds \
                 with HTTP 402, it pays the Lightning invoice and retries. Use l402_request \
                 to access paid APIs, check_budget to monitor spending, and list_receipts \
                 for payment audit trails.",
            )
    }
}
