ALTER TABLE mqtt_broker_auths
    DROP COLUMN subscribe_topic;

ALTER TABLE pending_producer_registrations
    DROP COLUMN mqtt_subscribe_topic;
