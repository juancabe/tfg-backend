CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Function to auto-update timestamps
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- 1. Users
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    login TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    refreshed_token_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_users_login UNIQUE (login)
);

CREATE TRIGGER update_users_updated_at
    BEFORE UPDATE ON users
    FOR EACH ROW
    EXECUTE PROCEDURE update_updated_at_column();

-- 2. Auth Methods
CREATE TABLE auth_methods (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    is_primary_auth BOOLEAN NOT NULL DEFAULT FALSE,
    kind SMALLINT NOT NULL, -- Changed from BIGINT
    auth_data JSONB NOT NULL
);
CREATE INDEX idx_auth_methods_data ON auth_methods USING GIN (auth_data);

-- 3. Refresh Tokens
CREATE TABLE refresh_tokens (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    claimed BOOLEAN NOT NULL DEFAULT FALSE,
    token_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    user_agent TEXT NOT NULL,
    ip_address TEXT NOT NULL,
    CONSTRAINT uq_refresh_tokens_hash UNIQUE (token_hash)
);

-- 4. Locations
CREATE TABLE locations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT,
    geo_lat DOUBLE PRECISION,
    geo_long DOUBLE PRECISION,
    CONSTRAINT chk_geo_lat CHECK (geo_lat >= -90 AND geo_lat <= 90),
    CONSTRAINT chk_geo_long CHECK (geo_long >= -180 AND geo_long <= 180)
);

-- 5. Groups
CREATE TABLE groups (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    location_id UUID NOT NULL REFERENCES locations(id),
    is_personal_group BOOLEAN NOT NULL DEFAULT FALSE,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    modification SMALLINT NOT NULL, -- Changed from BIGINT
    visibility SMALLINT NOT NULL    -- Changed from BIGINT
);

-- 6. Group Members
CREATE TABLE group_members (
    group_id UUID NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    PRIMARY KEY (group_id, user_id)
);

-- 7. Group Join Requests
CREATE TABLE group_join_requests (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    solicitor_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    target_group_id UUID NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    accepted_at TIMESTAMPTZ,
    declined_at TIMESTAMPTZ
);

-- 8. Favorite Groups
CREATE TABLE favorite_groups (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    group_id UUID NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, group_id)
);

-- 9. Producers
CREATE TABLE producers (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    location_id UUID NOT NULL REFERENCES locations(id),
    main_group_id UUID NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL,
    last_seen TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    forwarder_id UUID REFERENCES producers(id) ON DELETE SET NULL,
);

CREATE TRIGGER update_producers_updated_at
    BEFORE UPDATE ON producers
    FOR EACH ROW
    EXECUTE PROCEDURE update_updated_at_column();

-- 10. Favorite Producers
CREATE TABLE favorite_producers (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    producer_id UUID NOT NULL REFERENCES producers(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, producer_id)
);

-- 11. Producers For Groups
CREATE TABLE producers_for_groups (
    producer_id UUID NOT NULL REFERENCES producers(id) ON DELETE CASCADE,
    group_id UUID NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    PRIMARY KEY (producer_id, group_id)
);

-- 12. Internet Auth
CREATE TABLE internet_auths (
    producer_id UUID PRIMARY KEY REFERENCES producers(id) ON DELETE CASCADE,
    x25519_pub_key_bytes_base64 TEXT NOT NULL
);

-- 13. MQTT Broker Auth
CREATE TABLE mqtt_broker_auths (
    producer_id UUID PRIMARY KEY REFERENCES producers(id) ON DELETE CASCADE,
    broker_host TEXT NOT NULL,
    broker_port SMALLINT NOT NULL,
    username TEXT NOT NULL,
    password TEXT NOT NULL,
    topic TEXT NOT NULL
);

-- 14. Commands
CREATE TABLE commands (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    target_producer_id UUID NOT NULL REFERENCES producers(id) ON DELETE CASCADE,
    status SMALLINT NOT NULL, -- Changed from BIGINT
    kind SMALLINT NOT NULL,   -- Changed from BIGINT
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    payload JSONB NOT NULL
);

-- 15. Packet Templates
CREATE TABLE packet_templates (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    creator_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    name TEXT NOT NULL
);

-- 16. Template Fields
CREATE TABLE template_fields (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    name TEXT NOT NULL,
    repr_enum_value SMALLINT NOT NULL, -- Changed from BIGINT
    unit_enum_value SMALLINT NOT NULL, -- Changed from BIGINT
    unit_enum_other_payload TEXT 
);

-- 17. Template Field Mappings
CREATE TABLE template_field_mappings (
    template_id UUID NOT NULL REFERENCES packet_templates(id) ON DELETE CASCADE,
    field_id UUID NOT NULL REFERENCES template_fields(id) ON DELETE CASCADE,
    json_key TEXT NOT NULL,
    PRIMARY KEY (template_id, field_id)
);

-- 18. Data
CREATE TABLE data (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    producer_id UUID NOT NULL REFERENCES producers(id) ON DELETE CASCADE,
    template_id UUID NOT NULL REFERENCES packet_templates(id),
    recv_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    payload JSONB NOT NULL
);

-- Indexes
CREATE INDEX idx_auth_methods_user_id ON auth_methods(user_id);
CREATE INDEX idx_refresh_tokens_user_id ON refresh_tokens(user_id);
CREATE INDEX idx_groups_owner_id ON groups(owner_user_id);
CREATE INDEX idx_groups_location_id ON groups(location_id);
CREATE INDEX idx_producers_location_id ON producers(location_id);
CREATE INDEX idx_producers_main_group_id ON producers(main_group_id);
CREATE INDEX idx_producers_forwarder_id ON producers(forwarder_id);
CREATE INDEX idx_data_producer_id ON data(producer_id);
CREATE INDEX idx_data_recv_at ON data(recv_at);
CREATE INDEX idx_commands_target_producer_id ON commands(target_producer_id);
CREATE INDEX idx_data_payload ON data USING GIN (payload);
