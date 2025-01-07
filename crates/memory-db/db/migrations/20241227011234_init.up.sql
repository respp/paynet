-- Keyset

CREATE TABLE IF NOT EXISTS keyset (
    id BIGINT PRIMARY KEY,
    unit TEXT NOT NULL,
    active BOOL NOT NULL,
    max_order INT2 NOT NULL,
    derivation_path_index INT4 NOT NULL,
    input_fee_ppk INT2 NOT NULL
);

CREATE INDEX IF NOT EXISTS keyset_unit_index ON keyset(unit);
CREATE INDEX IF NOT EXISTS keyset_active_index ON keyset(active);

-- Blind Signature

CREATE TABLE blind_signature (
    y BYTEA CHECK (length(y) = 33) PRIMARY KEY,
    amount INT8 NOT NULL,
    keyset_id BIGINT REFERENCES keyset(id) NOT NULL,
    c BYTEA CHECK (length(c) = 33) NOT NULL
);

CREATE INDEX IF NOT EXISTS blind_signature_keyset_id_index ON blind_signature(keyset_id);

-- Proof 

CREATE TABLE IF NOT EXISTS proof (
    y BYTEA CHECK (length(y) = 33) PRIMARY KEY,
    amount INT8 NOT NULL,
    keyset_id BIGINT REFERENCES keyset(id) NOT NULL,
    secret TEXT NOT NULL,
    c BYTEA CHECK (length(c) = 33) NOT NULL,
    state INT2 NOT NULL
);

CREATE INDEX IF NOT EXISTS proof_state_index ON proof(state);
CREATE INDEX IF NOT EXISTS proof_secret_index ON proof(secret);
