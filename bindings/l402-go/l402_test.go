package l402

import (
	"testing"
)

func TestMockServerLifecycle(t *testing.T) {
	server, err := NewMockServer(map[string]uint64{
		"/api/data": 10,
	})
	if err != nil {
		t.Fatalf("NewMockServer failed: %v", err)
	}
	defer server.Close()

	url := server.URL()
	if url == "" {
		t.Fatal("server URL should not be empty")
	}
	t.Logf("Mock server running at: %s", url)
}

func TestMockServerEmptyEndpoints(t *testing.T) {
	_, err := NewMockServer(map[string]uint64{})
	if err == nil {
		t.Fatal("expected error for empty endpoints")
	}
}

func TestClientLifecycle(t *testing.T) {
	server, err := NewMockServer(map[string]uint64{
		"/api/data": 10,
	})
	if err != nil {
		t.Fatalf("NewMockServer failed: %v", err)
	}
	defer server.Close()

	client, err := NewMockClient(server, 100)
	if err != nil {
		t.Fatalf("NewMockClient failed: %v", err)
	}
	defer client.Close()

	if spent := client.TotalSpent(); spent != 0 {
		t.Fatalf("expected 0 spent, got %d", spent)
	}
}

func TestGetRequestWithPayment(t *testing.T) {
	server, err := NewMockServer(map[string]uint64{
		"/api/data": 10,
	})
	if err != nil {
		t.Fatalf("NewMockServer failed: %v", err)
	}
	defer server.Close()

	client, err := NewMockClient(server, 100)
	if err != nil {
		t.Fatalf("NewMockClient failed: %v", err)
	}
	defer client.Close()

	resp, err := client.Get(server.URL() + "/api/data")
	if err != nil {
		t.Fatalf("Get failed: %v", err)
	}

	if resp.Status != 200 {
		t.Errorf("expected status 200, got %d", resp.Status)
	}

	if !resp.Paid {
		t.Error("expected paid=true")
	}

	if resp.Receipt == nil {
		t.Fatal("expected receipt, got nil")
	}

	if resp.Receipt.AmountSats == 0 {
		t.Error("expected non-zero amount in receipt")
	}

	if resp.Receipt.PaymentHash == "" {
		t.Error("expected non-empty payment hash")
	}

	if resp.Receipt.Preimage == "" {
		t.Error("expected non-empty preimage")
	}

	t.Logf("Response: status=%d, paid=%v, amount=%d sats, fee=%d sats",
		resp.Status, resp.Paid, resp.Receipt.AmountSats, resp.Receipt.FeeSats)
}

func TestTotalSpentAfterPayment(t *testing.T) {
	server, err := NewMockServer(map[string]uint64{
		"/api/data": 10,
	})
	if err != nil {
		t.Fatalf("NewMockServer failed: %v", err)
	}
	defer server.Close()

	client, err := NewMockClient(server, 100)
	if err != nil {
		t.Fatalf("NewMockClient failed: %v", err)
	}
	defer client.Close()

	_, err = client.Get(server.URL() + "/api/data")
	if err != nil {
		t.Fatalf("Get failed: %v", err)
	}

	spent := client.TotalSpent()
	if spent == 0 {
		t.Error("expected non-zero total spent after payment")
	}
	t.Logf("Total spent: %d sats", spent)
}

func TestReceipts(t *testing.T) {
	server, err := NewMockServer(map[string]uint64{
		"/api/data": 10,
	})
	if err != nil {
		t.Fatalf("NewMockServer failed: %v", err)
	}
	defer server.Close()

	client, err := NewMockClient(server, 100)
	if err != nil {
		t.Fatalf("NewMockClient failed: %v", err)
	}
	defer client.Close()

	// No receipts initially
	receipts, err := client.Receipts()
	if err != nil {
		t.Fatalf("Receipts failed: %v", err)
	}
	if len(receipts) != 0 {
		t.Errorf("expected 0 receipts, got %d", len(receipts))
	}

	// Make a request
	_, err = client.Get(server.URL() + "/api/data")
	if err != nil {
		t.Fatalf("Get failed: %v", err)
	}

	// Should have one receipt
	receipts, err = client.Receipts()
	if err != nil {
		t.Fatalf("Receipts failed: %v", err)
	}
	if len(receipts) != 1 {
		t.Fatalf("expected 1 receipt, got %d", len(receipts))
	}

	r := receipts[0]
	if r.AmountSats == 0 {
		t.Error("expected non-zero amount")
	}
	if r.PaymentHash == "" {
		t.Error("expected non-empty payment hash")
	}
	if r.ResponseStatus != 200 {
		t.Errorf("expected response status 200, got %d", r.ResponseStatus)
	}
	if r.TotalCostSats() == 0 {
		t.Error("expected non-zero total cost")
	}
	t.Logf("Receipt: endpoint=%s, amount=%d, fee=%d, total=%d, hash=%s",
		r.Endpoint, r.AmountSats, r.FeeSats, r.TotalCostSats(), r.PaymentHash)
}

func TestMultipleEndpoints(t *testing.T) {
	server, err := NewMockServer(map[string]uint64{
		"/api/cheap":     5,
		"/api/expensive": 100,
	})
	if err != nil {
		t.Fatalf("NewMockServer failed: %v", err)
	}
	defer server.Close()

	client, err := NewMockClient(server, 200)
	if err != nil {
		t.Fatalf("NewMockClient failed: %v", err)
	}
	defer client.Close()

	resp1, err := client.Get(server.URL() + "/api/cheap")
	if err != nil {
		t.Fatalf("Get cheap failed: %v", err)
	}
	if resp1.Status != 200 {
		t.Errorf("expected 200, got %d", resp1.Status)
	}

	resp2, err := client.Get(server.URL() + "/api/expensive")
	if err != nil {
		t.Fatalf("Get expensive failed: %v", err)
	}
	if resp2.Status != 200 {
		t.Errorf("expected 200, got %d", resp2.Status)
	}

	receipts, err := client.Receipts()
	if err != nil {
		t.Fatalf("Receipts failed: %v", err)
	}
	if len(receipts) != 2 {
		t.Fatalf("expected 2 receipts, got %d", len(receipts))
	}
}

func TestReceiptTotalCost(t *testing.T) {
	r := &Receipt{
		AmountSats: 100,
		FeeSats:    5,
	}
	if r.TotalCostSats() != 105 {
		t.Errorf("expected 105, got %d", r.TotalCostSats())
	}
}

func TestNilClientError(t *testing.T) {
	_, err := NewMockClient(nil, 100)
	if err == nil {
		t.Fatal("expected error for nil server")
	}
}

func TestClosedClientError(t *testing.T) {
	server, err := NewMockServer(map[string]uint64{"/api/data": 10})
	if err != nil {
		t.Fatalf("NewMockServer failed: %v", err)
	}
	defer server.Close()

	client, err := NewMockClient(server, 100)
	if err != nil {
		t.Fatalf("NewMockClient failed: %v", err)
	}

	client.Close()

	_, err = client.Get(server.URL() + "/api/data")
	if err == nil {
		t.Fatal("expected error for closed client")
	}
}
