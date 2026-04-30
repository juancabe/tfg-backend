ALTER TABLE pending_producer_registrations
    DROP COLUMN forward_lora;

DROP INDEX IF EXISTS idx_producers_group_forward_lora;

ALTER TABLE producers
    DROP CONSTRAINT IF EXISTS chk_producers_lora_addr,
    DROP COLUMN IF EXISTS forward_lora,
    DROP COLUMN IF EXISTS lora_addr;
