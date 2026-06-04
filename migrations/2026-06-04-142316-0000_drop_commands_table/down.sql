CREATE TABLE commands (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    target_producer_id UUID NOT NULL REFERENCES producers(id) ON DELETE CASCADE,
    status SMALLINT NOT NULL CHECK (status IN (1, 2, 3, 4)),
    kind SMALLINT NOT NULL CHECK (kind IN (1, 2, 3)),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    payload JSONB NOT NULL
);
