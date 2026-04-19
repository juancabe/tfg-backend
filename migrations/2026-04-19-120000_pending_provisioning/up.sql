ALTER TABLE mqtt_broker_auths
    RENAME COLUMN password TO password_hash;

ALTER TABLE mqtt_broker_auths
    ADD COLUMN subscribe_topic TEXT NOT NULL DEFAULT '/v1/forbidden';

CREATE UNIQUE INDEX IF NOT EXISTS uq_mqtt_broker_auths_username
ON mqtt_broker_auths (username);

CREATE TABLE pending_producer_registrations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    owner_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    group_id UUID NOT NULL REFERENCES groups(id) ON DELETE CASCADE,
    producer_id UUID REFERENCES producers(id) ON DELETE SET NULL,

    status SMALLINT NOT NULL CHECK (status IN (1, 2, 3, 4)),
    expires_at TIMESTAMPTZ NOT NULL,
    confirmed_at TIMESTAMPTZ,
    cancelled_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    producer_name TEXT NOT NULL,
    ble_name TEXT NOT NULL,
    description TEXT NOT NULL,
    location_name TEXT,
    location_geo_lat DOUBLE PRECISION,
    location_geo_long DOUBLE PRECISION,
    forwarder_id UUID REFERENCES producers(id) ON DELETE SET NULL,
    data_template_id UUID REFERENCES packet_templates(id) ON DELETE SET NULL,

    mqtt_username TEXT NOT NULL,
    mqtt_password_hash TEXT NOT NULL,
    mqtt_topic TEXT NOT NULL,
    mqtt_subscribe_topic TEXT NOT NULL,
    broker_host TEXT NOT NULL,
    broker_port SMALLINT NOT NULL,

    x25519_pub_key_bytes_base64 TEXT,
    lora_addr INTEGER,

    CONSTRAINT chk_pending_loc_lat CHECK (location_geo_lat >= -90 AND location_geo_lat <= 90),
    CONSTRAINT chk_pending_loc_long CHECK (location_geo_long >= -180 AND location_geo_long <= 180),
    CONSTRAINT chk_pending_lora_addr CHECK (lora_addr IS NULL OR (lora_addr >= 0 AND lora_addr <= 65535))
);

CREATE UNIQUE INDEX uq_pending_mqtt_username ON pending_producer_registrations(mqtt_username);
CREATE INDEX idx_pending_owner_user_id ON pending_producer_registrations(owner_user_id);
CREATE INDEX idx_pending_group_id ON pending_producer_registrations(group_id);
CREATE INDEX idx_pending_status_expires_at ON pending_producer_registrations(status, expires_at);
