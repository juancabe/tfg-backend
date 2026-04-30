ALTER TABLE producers
    ADD COLUMN lora_addr INTEGER,
    ADD COLUMN forward_lora BOOLEAN NOT NULL DEFAULT FALSE,
    ADD CONSTRAINT chk_producers_lora_addr CHECK (lora_addr IS NULL OR (lora_addr >= 0 AND lora_addr <= 65535));

CREATE INDEX idx_producers_group_forward_lora ON producers(group_id, forward_lora);

ALTER TABLE pending_producer_registrations
    ADD COLUMN forward_lora BOOLEAN NOT NULL DEFAULT FALSE;
