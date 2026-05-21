ALTER TABLE groups ADD CONSTRAINT uq_groups_owner_name UNIQUE (owner_user_id, name);
ALTER TABLE producers ADD CONSTRAINT uq_producers_group_name UNIQUE (group_id, name);
