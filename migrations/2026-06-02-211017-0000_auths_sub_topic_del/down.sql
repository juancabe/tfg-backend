ALTER TABLE mqtt_broker_auths
    ADD COLUMN subscribe_topic TEXT NOT NULL DEFAULT '/v1/forbidden';

ALTER TABLE pending_producer_registrations
    ADD COLUMN mqtt_subscribe_topic TEXT NOT NULL DEFAULT '/v1/forbidden';
