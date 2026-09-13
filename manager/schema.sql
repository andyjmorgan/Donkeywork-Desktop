CREATE TABLE IF NOT EXISTS devices (
 id uuid PRIMARY KEY,
 name text NOT NULL CHECK (length(name) BETWEEN 1 AND 100),
 description text NOT NULL CHECK (length(description) <= 2000),
 created_at timestamptz NOT NULL DEFAULT now(),
 code_digest bytea UNIQUE,
 code_expires_at timestamptz NOT NULL,
 claimed_at timestamptz,
 revoked_at timestamptz,
 certificate_pem text,
 certificate_sha256 bytea UNIQUE,
 CHECK ((claimed_at IS NULL) = (certificate_pem IS NULL))
);
ALTER TABLE devices ADD COLUMN IF NOT EXISTS last_seen_at timestamptz;
