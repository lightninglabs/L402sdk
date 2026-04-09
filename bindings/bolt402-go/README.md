# bolt402-go

Go bindings for the [bolt402](https://github.com/lightninglabs/bolt402) L402 client SDK.

bolt402 enables Go applications and AI agent frameworks to consume L402-gated APIs with automatic Lightning payments. The core engine is written in Rust; this package calls into it via CGo FFI.

## Prerequisites

- Go 1.21+
- Rust toolchain (for building the FFI library)
- C compiler (gcc/clang, for CGo)

## Building

First, build the Rust FFI static library:

```bash
# From the bolt402 repo root
cargo build -p bolt402-ffi --release

# Copy the library to the Go bindings lib/ directory
mkdir -p bindings/bolt402-go/lib
cp target/release/libbolt402_ffi.a bindings/bolt402-go/lib/
```

Then build and test:

```bash
cd bindings/bolt402-go
go test -v ./...
```

## Usage

```go
package main

import (
    "fmt"
    "log"

    bolt402 "github.com/lightninglabs/bolt402/bindings/bolt402-go"
)

func main() {
    // Create a mock server for testing
    server, err := bolt402.NewMockServer(map[string]uint64{
        "/api/data": 10, // 10 sats per request
    })
    if err != nil {
        log.Fatal(err)
    }
    defer server.Close()

    // Create a client connected to the mock server
    client, err := bolt402.NewMockClient(server, 100) // max 100 sats fee
    if err != nil {
        log.Fatal(err)
    }
    defer client.Close()

    // Make an L402-gated request — payment happens automatically
    resp, err := client.Get(server.URL() + "/api/data")
    if err != nil {
        log.Fatal(err)
    }

    fmt.Printf("Status: %d\n", resp.Status)
    fmt.Printf("Paid: %v\n", resp.Paid)
    fmt.Printf("Body: %s\n", resp.Body)

    if resp.Receipt != nil {
        fmt.Printf("Amount: %d sats\n", resp.Receipt.AmountSats)
        fmt.Printf("Fee: %d sats\n", resp.Receipt.FeeSats)
    }

    // Check total spending
    fmt.Printf("Total spent: %d sats\n", client.TotalSpent())

    // Get detailed receipts
    receipts, err := client.Receipts()
    if err != nil {
        log.Fatal(err)
    }
    for _, r := range receipts {
        fmt.Printf("Receipt: %s — %d sats (+ %d fee)\n",
            r.Endpoint, r.AmountSats, r.FeeSats)
    }
}
```

## API

### Types

- **`MockServer`** — Mock L402 server for testing
- **`Client`** — L402 client that handles payment challenges
- **`Response`** — HTTP response with payment metadata
- **`Receipt`** — Payment receipt with amount, fee, hash, preimage

### Functions

- `NewMockServer(endpoints map[string]uint64) (*MockServer, error)`
- `NewMockClient(server *MockServer, maxFeeSats uint64) (*Client, error)`
- `(*Client).Get(url string) (*Response, error)`
- `(*Client).Post(url, body string) (*Response, error)`
- `(*Client).TotalSpent() uint64`
- `(*Client).Receipts() ([]Receipt, error)`
- `(*Client).Close()`
- `(*MockServer).URL() string`
- `(*MockServer).Close()`

## Architecture

```
bolt402-core (Rust)
      ↓
bolt402-ffi (Rust cdylib/staticlib, extern "C")
      ↓  C ABI
bolt402-go (Go package via CGo)
      ↓
Your Go application
```

## License

MIT OR Apache-2.0
