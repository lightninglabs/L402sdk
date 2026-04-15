/**
 * Simple backend HTTP server for Aperture to proxy to.
 *
 * This server has no authentication logic — Aperture handles all L402
 * challenge/response flow. By the time a request reaches this server,
 * the client has already paid.
 *
 * Endpoints:
 *   GET /health         - Health check (not proxied through Aperture)
 *   GET /api/data       - Returns JSON data (100 sats via Aperture)
 *   GET /api/premium    - Returns premium JSON data (500 sats via Aperture)
 *   GET /api/cheap      - Returns cheap JSON data (10 sats via Aperture)
 */

const http = require("http");

const PORT = parseInt(process.env.BACKEND_PORT || "9000", 10);

const ROUTES = {
  "/health": () => ({ status: "ok", service: "l402-regtest-backend" }),
  "/api/data": () => ({
    ok: true,
    resource: "data",
    message: "This is a protected resource (100 sats)",
  }),
  "/api/premium": () => ({
    ok: true,
    resource: "premium",
    message: "This is a premium resource (500 sats)",
  }),
  "/api/cheap": () => ({
    ok: true,
    resource: "cheap",
    message: "This is a cheap resource (10 sats)",
  }),
};

const server = http.createServer((req, res) => {
  const url = new URL(req.url, `http://${req.headers.host}`);
  const handler = ROUTES[url.pathname];

  if (!handler) {
    res.writeHead(404, { "Content-Type": "application/json" });
    res.end(JSON.stringify({ error: "not found" }));
    return;
  }

  const body = handler();
  res.writeHead(200, { "Content-Type": "application/json" });
  res.end(JSON.stringify(body));
});

server.listen(PORT, "0.0.0.0", () => {
  console.log(`Backend server listening on :${PORT}`);
  console.log(`Routes: ${Object.keys(ROUTES).join(", ")}`);
});
