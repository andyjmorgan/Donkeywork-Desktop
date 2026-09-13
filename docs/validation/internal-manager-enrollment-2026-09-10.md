# Internal manager enrollment slice

Local implementation at manager/; no fleet services replaced or restarted.
Baseline repository HEAD: 50b41a66ccbab35b85067edd89dec7d6751bf3c3. Existing dirty
worktree preserved. No commit/push claimed.

`MANAGER_TEST_DATABASE_URL=... go test -race -v ./...` passed against PostgreSQL
17 in isolated container dwdesktop-manager-pg-test-20260910, loopback port 55439.
Integration used a uniquely named schema, dropped on completion. The database
container was stopped after testing and can be restarted; no lab database used.

Five tests passed: code generation/validation; signed CSR parsing; PostgreSQL
enrollment; HTTP TLS/Host/Origin/body boundary; per-peer attempt limit. Database
test includes 16 concurrent claims with exactly one winner, repeat claim denial,
exact 24-hour expiry, expired/revoked rejection, invalid CSR leaving code usable,
and CA-signed manager-selected identity rather than requested CSR subject/SAN.

This validates the transactional enrollment implementation, not deployed HTTPS,
browser UX, certificate renewal, revocation propagation, device mTLS connectivity
or live online/session inventory. Those remain subsequent delivery slices.
