// Package bolt402 provides Go bindings for the bolt402 L402 client SDK.
//
// bolt402 enables Go applications and AI agent frameworks to consume
// L402-gated APIs with automatic Lightning payments. The core engine is
// written in Rust; this package calls into it via CGo FFI.
//
// # Quick Start
//
//	server, _ := bolt402.NewMockServer(map[string]uint64{
//	    "/api/data": 10,
//	})
//	defer server.Close()
//
//	client, _ := bolt402.NewMockClient(server, 100)
//	defer client.Close()
//
//	resp, _ := client.Get(server.URL() + "/api/data")
//	fmt.Println(resp.Status, resp.Paid, resp.Body)
package bolt402

/*
#cgo LDFLAGS: -L${SRCDIR}/lib -lbolt402_ffi -lm -ldl -lpthread
#include "../../crates/bolt402-ffi/include/bolt402.h"
#include <stdlib.h>
*/
import "C"
import (
	"encoding/json"
	"errors"
	"fmt"
	"runtime"
	"unsafe"
)

// Receipt represents a payment receipt for an L402 transaction.
type Receipt struct {
	Timestamp      uint64 `json:"timestamp"`
	Endpoint       string `json:"endpoint"`
	AmountSats     uint64 `json:"amount_sats"`
	FeeSats        uint64 `json:"fee_sats"`
	PaymentHash    string `json:"payment_hash"`
	Preimage       string `json:"preimage"`
	ResponseStatus uint16 `json:"response_status"`
	LatencyMs      uint64 `json:"latency_ms"`
}

// TotalCostSats returns the total cost (amount + fee) in satoshis.
func (r *Receipt) TotalCostSats() uint64 {
	return r.AmountSats + r.FeeSats
}

// Response represents the result of an L402-aware HTTP request.
type Response struct {
	// Status is the HTTP status code.
	Status uint16
	// Paid indicates whether a Lightning payment was made.
	Paid bool
	// Body is the response body as a string.
	Body string
	// Receipt is the payment receipt, if a payment was made.
	Receipt *Receipt
}

// MockServer is a mock L402 server for testing and development.
type MockServer struct {
	ptr *C.Bolt402MockServer
}

// NewMockServer creates a mock L402 server with the given endpoints.
// Each key is a URL path (e.g. "/api/data") and the value is the price
// in satoshis.
func NewMockServer(endpoints map[string]uint64) (*MockServer, error) {
	if len(endpoints) == 0 {
		return nil, errors.New("bolt402: at least one endpoint is required")
	}

	cEndpoints := make([]C.Bolt402Endpoint, 0, len(endpoints))
	cStrings := make([]*C.char, 0, len(endpoints))

	for path, price := range endpoints {
		cPath := C.CString(path)
		cStrings = append(cStrings, cPath)
		cEndpoints = append(cEndpoints, C.Bolt402Endpoint{
			path:       cPath,
			price_sats: C.uint64_t(price),
		})
	}

	ptr := C.bolt402_mock_server_new(
		&cEndpoints[0],
		C.uintptr_t(len(cEndpoints)),
	)

	// Free the C strings after the server has copied them
	for _, cs := range cStrings {
		C.free(unsafe.Pointer(cs))
	}

	if ptr == nil {
		return nil, lastError("failed to create mock server")
	}

	server := &MockServer{ptr: ptr}
	runtime.SetFinalizer(server, (*MockServer).Close)
	return server, nil
}

// URL returns the base URL of the mock server.
func (s *MockServer) URL() string {
	if s.ptr == nil {
		return ""
	}
	return C.GoString(C.bolt402_mock_server_url(s.ptr))
}

// Close frees the mock server resources.
func (s *MockServer) Close() {
	if s.ptr != nil {
		C.bolt402_mock_server_free(s.ptr)
		s.ptr = nil
	}
}

// Client is an L402 client that handles the full payment-gated HTTP flow.
type Client struct {
	ptr *C.Bolt402Client
}

// NewMockClient creates a client connected to a mock server.
func NewMockClient(server *MockServer, maxFeeSats uint64) (*Client, error) {
	if server == nil || server.ptr == nil {
		return nil, errors.New("bolt402: server is nil")
	}

	ptr := C.bolt402_client_new_mock(server.ptr, C.uint64_t(maxFeeSats))
	if ptr == nil {
		return nil, lastError("failed to create client")
	}

	client := &Client{ptr: ptr}
	runtime.SetFinalizer(client, (*Client).Close)
	return client, nil
}

// Get sends a GET request, automatically handling L402 payment challenges.
func (c *Client) Get(url string) (*Response, error) {
	if c.ptr == nil {
		return nil, errors.New("bolt402: client is closed")
	}

	cURL := C.CString(url)
	defer C.free(unsafe.Pointer(cURL))

	resp := C.bolt402_client_get(c.ptr, cURL)
	if resp == nil {
		return nil, lastError("GET request failed")
	}
	defer C.bolt402_response_free(resp)

	return extractResponse(resp), nil
}

// Post sends a POST request with an optional body, handling L402 challenges.
func (c *Client) Post(url, body string) (*Response, error) {
	if c.ptr == nil {
		return nil, errors.New("bolt402: client is closed")
	}

	cURL := C.CString(url)
	defer C.free(unsafe.Pointer(cURL))

	var cBody *C.char
	if body != "" {
		cBody = C.CString(body)
		defer C.free(unsafe.Pointer(cBody))
	}

	resp := C.bolt402_client_post(c.ptr, cURL, cBody)
	if resp == nil {
		return nil, lastError("POST request failed")
	}
	defer C.bolt402_response_free(resp)

	return extractResponse(resp), nil
}

// TotalSpent returns the total amount spent by the client in satoshis.
func (c *Client) TotalSpent() uint64 {
	if c.ptr == nil {
		return 0
	}
	return uint64(C.bolt402_client_total_spent(c.ptr))
}

// Receipts returns all payment receipts recorded by the client.
func (c *Client) Receipts() ([]Receipt, error) {
	if c.ptr == nil {
		return nil, errors.New("bolt402: client is closed")
	}

	cJSON := C.bolt402_client_receipts_json(c.ptr)
	if cJSON == nil {
		return nil, lastError("failed to get receipts")
	}
	defer C.bolt402_string_free(cJSON)

	jsonStr := C.GoString(cJSON)
	var receipts []Receipt
	if err := json.Unmarshal([]byte(jsonStr), &receipts); err != nil {
		return nil, fmt.Errorf("bolt402: failed to parse receipts JSON: %w", err)
	}

	return receipts, nil
}

// Close frees the client resources.
func (c *Client) Close() {
	if c.ptr != nil {
		C.bolt402_client_free(c.ptr)
		c.ptr = nil
	}
}

func extractResponse(resp *C.Bolt402Response) *Response {
	r := &Response{
		Status: uint16(C.bolt402_response_status(resp)),
		Paid:   bool(C.bolt402_response_paid(resp)),
		Body:   C.GoString(C.bolt402_response_body(resp)),
	}

	if bool(C.bolt402_response_has_receipt(resp)) {
		hashPtr := C.bolt402_response_receipt_payment_hash(resp)
		preimagePtr := C.bolt402_response_receipt_preimage(resp)

		r.Receipt = &Receipt{
			AmountSats:  uint64(C.bolt402_response_receipt_amount_sats(resp)),
			FeeSats:     uint64(C.bolt402_response_receipt_fee_sats(resp)),
			PaymentHash: C.GoString(hashPtr),
			Preimage:    C.GoString(preimagePtr),
		}
	}

	return r
}

func lastError(fallback string) error {
	msg := C.bolt402_last_error_message()
	if msg != nil {
		errStr := C.GoString(msg)
		C.bolt402_string_free((*C.char)(unsafe.Pointer(msg)))
		return fmt.Errorf("bolt402: %s", errStr)
	}
	return fmt.Errorf("bolt402: %s", fallback)
}
