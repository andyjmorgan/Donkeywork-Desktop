package manager

import (
	"context"
	"crypto"
	"crypto/ecdsa"
	"crypto/hmac"
	"crypto/rand"
	"crypto/rsa"
	"crypto/sha256"
	"crypto/x509"
	"crypto/x509/pkix"
	_ "embed"
	"encoding/hex"
	"encoding/pem"
	"errors"
	"fmt"
	"math/big"
	"net/url"
	"strings"
	"time"

	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"
)

//go:embed schema.sql
var schema string

var ErrEnrollment = errors.New("enrollment rejected")

func (s *Store) Regenerate(ctx context.Context, id string) (Enrollment, error) {
	code, err := newCode()
	if err != nil {
		return Enrollment{}, err
	}
	hash, _ := digest(s.pepper, code)
	var d Device
	err = s.pool.QueryRow(ctx, `UPDATE devices SET code_digest=$2,code_expires_at=now()+interval '24 hours'
 WHERE id=$1 AND claimed_at IS NULL AND revoked_at IS NULL RETURNING id::text,name,description,created_at,code_expires_at,claimed_at,revoked_at`, id, hash).Scan(&d.ID, &d.Name, &d.Description, &d.CreatedAt, &d.ExpiresAt, &d.ClaimedAt, &d.RevokedAt)
	if errors.Is(err, pgx.ErrNoRows) {
		return Enrollment{}, ErrEnrollment
	}
	return Enrollment{d, code}, err
}
func (s *Store) DeleteRevoked(ctx context.Context, id string) error {
	result, err := s.pool.Exec(ctx, `DELETE FROM devices WHERE id=$1 AND revoked_at IS NOT NULL`, id)
	if err != nil {
		return err
	}
	if result.RowsAffected() != 1 {
		return ErrEnrollment
	}
	return nil
}
func (s *Store) Revoke(ctx context.Context, id string) error {
	result, err := s.pool.Exec(ctx, `UPDATE devices SET revoked_at=COALESCE(revoked_at,now()),code_digest=NULL WHERE id=$1`, id)
	if err != nil {
		return err
	}
	if result.RowsAffected() != 1 {
		return ErrEnrollment
	}
	return nil
}

const alphabet = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789"

type Device struct {
	ID          string     `json:"id"`
	Name        string     `json:"name"`
	Description string     `json:"description"`
	CreatedAt   time.Time  `json:"createdAt"`
	ExpiresAt   time.Time  `json:"codeExpiresAt"`
	ClaimedAt   *time.Time `json:"claimedAt"`
	RevokedAt   *time.Time `json:"revokedAt"`
}
type Enrollment struct {
	Device Device `json:"device"`
	Code   string `json:"code"`
}
type Store struct {
	pool   *pgxpool.Pool
	pepper []byte
	ca     *x509.Certificate
	key    crypto.Signer
}

func NewStore(pool *pgxpool.Pool, pepper []byte, ca *x509.Certificate, key crypto.Signer) (*Store, error) {
	if pool == nil || len(pepper) < 32 || ca == nil || key == nil || !ca.IsCA || ca.KeyUsage&x509.KeyUsageCertSign == 0 {
		return nil, errors.New("invalid enrollment configuration")
	}
	a, err := x509.MarshalPKIXPublicKey(ca.PublicKey)
	if err != nil {
		return nil, err
	}
	b, err := x509.MarshalPKIXPublicKey(key.Public())
	if err != nil {
		return nil, err
	}
	if !hmac.Equal(a, b) {
		return nil, errors.New("CA key mismatch")
	}
	return &Store{pool, append([]byte(nil), pepper...), ca, key}, nil
}
func (s *Store) Migrate(ctx context.Context) error { _, err := s.pool.Exec(ctx, schema); return err }

func digest(pepper []byte, code string) ([]byte, error) {
	code = strings.ToUpper(strings.TrimSpace(code))
	if len(code) == 9 && code[4] == '-' {
		code = code[:4] + code[5:]
	}
	if len(code) != 8 {
		return nil, ErrEnrollment
	}
	for _, c := range code {
		if !strings.ContainsRune(alphabet, c) {
			return nil, ErrEnrollment
		}
	}
	mac := hmac.New(sha256.New, pepper)
	mac.Write([]byte(code))
	return mac.Sum(nil), nil
}
func newCode() (string, error) {
	var b [8]byte
	// Alphabet has 32 entries: a mask is uniform, without modulo bias.
	if _, err := rand.Read(b[:]); err != nil {
		return "", err
	}
	for i := range b {
		b[i] = alphabet[int(b[i])%len(alphabet)]
	}
	return string(b[:4]) + "-" + string(b[4:]), nil
}
func newID() (string, error) {
	var b [16]byte
	if _, err := rand.Read(b[:]); err != nil {
		return "", err
	}
	b[6] = (b[6] & 15) | 64
	b[8] = (b[8] & 63) | 128
	h := hex.EncodeToString(b[:])
	return h[:8] + "-" + h[8:12] + "-" + h[12:16] + "-" + h[16:20] + "-" + h[20:], nil
}
func (s *Store) Create(ctx context.Context, name, description string) (Enrollment, error) {
	name = strings.TrimSpace(name)
	if len(name) == 0 || len(name) > 100 || len(description) > 2000 {
		return Enrollment{}, errors.New("invalid device details")
	}
	id, err := newID()
	if err != nil {
		return Enrollment{}, err
	}
	code, err := newCode()
	if err != nil {
		return Enrollment{}, err
	}
	hash, _ := digest(s.pepper, code)
	d := Device{ID: id, Name: name, Description: description}
	err = s.pool.QueryRow(ctx, `INSERT INTO devices(id,name,description,code_digest,code_expires_at)
 VALUES($1,$2,$3,$4,now()+interval '24 hours') RETURNING created_at,code_expires_at`, id, name, description, hash).Scan(&d.CreatedAt, &d.ExpiresAt)
	return Enrollment{d, code}, err
}
func (s *Store) List(ctx context.Context) ([]Device, error) {
	rows, err := s.pool.Query(ctx, `SELECT id::text,name,description,created_at,code_expires_at,claimed_at,revoked_at FROM devices ORDER BY created_at,id`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	devices := []Device{}
	for rows.Next() {
		var d Device
		if err = rows.Scan(&d.ID, &d.Name, &d.Description, &d.CreatedAt, &d.ExpiresAt, &d.ClaimedAt, &d.RevokedAt); err != nil {
			return nil, err
		}
		devices = append(devices, d)
	}
	return devices, rows.Err()
}

func parseCSR(raw string) (*x509.CertificateRequest, error) {
	if len(raw) > 16384 {
		return nil, ErrEnrollment
	}
	block, rest := pem.Decode([]byte(raw))
	if block == nil || block.Type != "CERTIFICATE REQUEST" || len(strings.TrimSpace(string(rest))) != 0 {
		return nil, ErrEnrollment
	}
	csr, err := x509.ParseCertificateRequest(block.Bytes)
	if err != nil {
		return nil, ErrEnrollment
	}
	if csr.CheckSignature() != nil {
		return nil, ErrEnrollment
	}
	switch k := csr.PublicKey.(type) {
	case *ecdsa.PublicKey:
		if k.Curve.Params().BitSize < 256 {
			return nil, ErrEnrollment
		}
	case *rsa.PublicKey:
		if k.N.BitLen() < 3072 {
			return nil, ErrEnrollment
		}
	default:
		return nil, ErrEnrollment
	}
	return csr, nil
}
func (s *Store) sign(id string, csr *x509.CertificateRequest) ([]byte, error) {
	now := time.Now().UTC()
	if now.Before(s.ca.NotBefore) || !now.Add(24*time.Hour).Before(s.ca.NotAfter) {
		return nil, errors.New("device CA requires renewal")
	}
	serial, err := rand.Int(rand.Reader, new(big.Int).Lsh(big.NewInt(1), 128))
	if err != nil {
		return nil, err
	}
	until := now.Add(30 * 24 * time.Hour)
	if until.After(s.ca.NotAfter) {
		until = s.ca.NotAfter
	}
	identity := &url.URL{Scheme: "spiffe", Host: "desktops.donkeywork.dev", Path: "/device/" + id}
	template := &x509.Certificate{SerialNumber: serial, Subject: pkix.Name{CommonName: id}, NotBefore: now.Add(-time.Minute), NotAfter: until,
		KeyUsage: x509.KeyUsageDigitalSignature, ExtKeyUsage: []x509.ExtKeyUsage{x509.ExtKeyUsageClientAuth}, URIs: []*url.URL{identity}}
	return x509.CreateCertificate(rand.Reader, template, s.ca, csr.PublicKey, s.key)
}
func (s *Store) Claim(ctx context.Context, code, rawCSR string) (string, string, error) {
	hash, err := digest(s.pepper, code)
	if err != nil {
		return "", "", ErrEnrollment
	}
	csr, err := parseCSR(rawCSR)
	if err != nil {
		return "", "", err
	}
	tx, err := s.pool.Begin(ctx)
	if err != nil {
		return "", "", err
	}
	defer tx.Rollback(context.Background())
	var id string
	err = tx.QueryRow(ctx, `SELECT id::text FROM devices WHERE code_digest=$1 AND claimed_at IS NULL
 AND revoked_at IS NULL AND code_expires_at>clock_timestamp() FOR UPDATE`, hash).Scan(&id)
	if errors.Is(err, pgx.ErrNoRows) {
		return "", "", ErrEnrollment
	}
	if err != nil {
		return "", "", err
	}
	der, err := s.sign(id, csr)
	if err != nil {
		return "", "", err
	}
	cert := string(pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: der}))
	fingerprint := sha256.Sum256(der)
	result, err := tx.Exec(ctx, `UPDATE devices SET claimed_at=clock_timestamp(),code_digest=NULL,certificate_pem=$2,certificate_sha256=$3
 WHERE id=$1 AND claimed_at IS NULL AND revoked_at IS NULL AND code_expires_at>clock_timestamp()`, id, cert, fingerprint[:])
	if err != nil {
		return "", "", err
	}
	if result.RowsAffected() != 1 {
		return "", "", ErrEnrollment
	}
	if err = tx.Commit(ctx); err != nil {
		return "", "", fmt.Errorf("enrollment commit uncertain: %w", err)
	}
	return id, cert, nil
}
