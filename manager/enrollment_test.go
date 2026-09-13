package manager

import (
	"context"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"math/big"
	"os"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/jackc/pgx/v5/pgxpool"
)

func TestCodes(t *testing.T) {
	if len(alphabet) != 32 {
		t.Fatal("alphabet must have 32 entries")
	}
	for n := 0; n < 100; n++ {
		code, err := newCode()
		if err != nil {
			t.Fatal(err)
		}
		if len(code) != 9 || code[4] != '-' {
			t.Fatal("bad format")
		}
		if _, err = digest([]byte("test"), code); err != nil {
			t.Fatal(err)
		}
	}
	for _, bad := range []string{"", "1234", "IIII-OOOO", "ABCD/EFGH"} {
		if _, err := digest(nil, bad); err == nil {
			t.Fatal("accepted malformed code")
		}
	}
}
func fixtures(t *testing.T) (*x509.Certificate, *ecdsa.PrivateKey, string) {
	t.Helper()
	key, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	template := &x509.Certificate{SerialNumber: big.NewInt(1), Subject: pkix.Name{CommonName: "test device CA"}, IsCA: true, BasicConstraintsValid: true, KeyUsage: x509.KeyUsageCertSign, NotBefore: time.Now().Add(-time.Hour), NotAfter: time.Now().Add(365 * 24 * time.Hour)}
	der, err := x509.CreateCertificate(rand.Reader, template, template, key.Public(), key)
	if err != nil {
		t.Fatal(err)
	}
	ca, err := x509.ParseCertificate(der)
	if err != nil {
		t.Fatal(err)
	}
	deviceKey, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	csr, err := x509.CreateCertificateRequest(rand.Reader, &x509.CertificateRequest{Subject: pkix.Name{CommonName: "untrusted-requested-name"}, DNSNames: []string{"evil.example"}}, deviceKey)
	if err != nil {
		t.Fatal(err)
	}
	return ca, key, string(pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE REQUEST", Bytes: csr}))
}
func TestCSR(t *testing.T) {
	_, _, csr := fixtures(t)
	if _, err := parseCSR(csr); err != nil {
		t.Fatal(err)
	}
	if _, err := parseCSR(csr + "trailing"); err == nil {
		t.Fatal("accepted trailing data")
	}
}

func TestPostgresEnrollment(t *testing.T) {
	dsn := os.Getenv("MANAGER_TEST_DATABASE_URL")
	if dsn == "" {
		t.Skip("set MANAGER_TEST_DATABASE_URL to a disposable database")
	}
	ctx := context.Background()
	pool, err := pgxpool.New(ctx, dsn)
	if err != nil {
		t.Fatal(err)
	}
	defer pool.Close()
	// A unique schema prevents deleting or colliding with unrelated test data.
	id, _ := newID()
	schemaName := "enrollment_"
	for _, c := range id {
		if c != '-' {
			schemaName += string(c)
		}
	}
	if _, err = pool.Exec(ctx, "CREATE SCHEMA "+schemaName); err != nil {
		t.Fatal(err)
	}
	defer pool.Exec(ctx, "DROP SCHEMA "+schemaName+" CASCADE")
	cfg, err := pgxpool.ParseConfig(dsn)
	if err != nil {
		t.Fatal(err)
	}
	cfg.ConnConfig.RuntimeParams["search_path"] = schemaName
	isolated, err := pgxpool.NewWithConfig(ctx, cfg)
	if err != nil {
		t.Fatal(err)
	}
	defer isolated.Close()
	ca, key, csr := fixtures(t)
	store, err := NewStore(isolated, make([]byte, 32), ca, key)
	if err != nil {
		t.Fatal(err)
	}
	if err = store.Migrate(ctx); err != nil {
		t.Fatal(err)
	}
	registration, err := store.Create(ctx, "test device", "description")
	if err != nil {
		t.Fatal(err)
	}
	if registration.Device.ExpiresAt.Sub(registration.Device.CreatedAt) != 24*time.Hour {
		t.Fatal("expiry is not 24 hours")
	}
	if _, _, err = store.Claim(ctx, registration.Code, "invalid CSR"); err != ErrEnrollment {
		t.Fatal("invalid CSR accepted")
	}
	var wins atomic.Int32
	var wg sync.WaitGroup
	for n := 0; n < 16; n++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			id, cert, err := store.Claim(ctx, registration.Code, csr)
			if err == ErrEnrollment {
				return
			}
			if err != nil {
				t.Error(err)
				return
			}
			wins.Add(1)
			block, _ := pem.Decode([]byte(cert))
			parsed, err := x509.ParseCertificate(block.Bytes)
			if err != nil {
				t.Error(err)
				return
			}
			if id != registration.Device.ID || parsed.Subject.CommonName != id || len(parsed.DNSNames) != 0 || len(parsed.URIs) != 1 {
				t.Error("certificate identity not manager assigned")
			}
			if err = parsed.CheckSignatureFrom(ca); err != nil {
				t.Error(err)
			}
		}()
	}
	wg.Wait()
	if wins.Load() != 1 {
		t.Fatalf("claim winners: %d", wins.Load())
	}
	if _, _, err = store.Claim(ctx, registration.Code, csr); err != ErrEnrollment {
		t.Fatal("claimed code reused")
	}
	expired, err := store.Create(ctx, "expired", "")
	if err != nil {
		t.Fatal(err)
	}
	if _, err = isolated.Exec(ctx, "UPDATE devices SET code_expires_at=now()-interval '1 second' WHERE id=$1", expired.Device.ID); err != nil {
		t.Fatal(err)
	}
	if _, _, err = store.Claim(ctx, expired.Code, csr); err != ErrEnrollment {
		t.Fatal("expired code accepted")
	}
	revoked, err := store.Create(ctx, "revoked", "")
	if err != nil {
		t.Fatal(err)
	}
	if _, err = isolated.Exec(ctx, "UPDATE devices SET revoked_at=now() WHERE id=$1", revoked.Device.ID); err != nil {
		t.Fatal(err)
	}
	if _, _, err = store.Claim(ctx, revoked.Code, csr); err != ErrEnrollment {
		t.Fatal("revoked code accepted")
	}
	devices, err := store.List(ctx)
	if err != nil || len(devices) != 3 {
		t.Fatalf("list: %v %d", err, len(devices))
	}
	if err = store.DeleteRevoked(ctx, registration.Device.ID); err != ErrEnrollment {
		t.Fatal("deleted enrolled device without revocation")
	}
	if err = store.DeleteRevoked(ctx, expired.Device.ID); err != ErrEnrollment {
		t.Fatal("deleted pending device without revocation")
	}
	if err = store.DeleteRevoked(ctx, revoked.Device.ID); err != nil {
		t.Fatal(err)
	}
	if err = store.DeleteRevoked(ctx, revoked.Device.ID); err != ErrEnrollment {
		t.Fatal("missing device delete should be rejected")
	}
	if _, _, err = store.Claim(ctx, revoked.Code, csr); err != ErrEnrollment {
		t.Fatal("deleted enrollment resurrected")
	}
	devices, err = store.List(ctx)
	if err != nil || len(devices) != 2 {
		t.Fatal("deleted record still listed")
	}
	testDeviceConnection(t, store)
}
