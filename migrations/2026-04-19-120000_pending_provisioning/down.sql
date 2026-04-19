DROP INDEX IF EXISTS idx_pending_status_expires_at;
DROP INDEX IF EXISTS idx_pending_group_id;
DROP INDEX IF EXISTS idx_pending_owner_user_id;
DROP INDEX IF EXISTS uq_pending_mqtt_username;

DROP TABLE IF EXISTS pending_producer_registrations;

DROP INDEX IF EXISTS uq_mqtt_broker_auths_username;

ALTER TABLE mqtt_broker_auths
    DROP COLUMN subscribe_topic;

ALTER TABLE mqtt_broker_auths
    RENAME COLUMN password_hash TO password;
