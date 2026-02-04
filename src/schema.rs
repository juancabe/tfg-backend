// @generated automatically by Diesel CLI.

diesel::table! {
    auth_methods (id) {
        id -> Uuid,
        user_id -> Uuid,
        is_primary_auth -> Bool,
        kind -> Int2,
        auth_data -> Jsonb,
    }
}

diesel::table! {
    commands (id) {
        id -> Uuid,
        target_producer_id -> Uuid,
        status -> Int2,
        kind -> Int2,
        created_at -> Timestamptz,
        payload -> Jsonb,
    }
}

diesel::table! {
    data (id) {
        id -> Uuid,
        producer_id -> Uuid,
        template_id -> Uuid,
        recv_at -> Timestamptz,
        payload -> Jsonb,
    }
}

diesel::table! {
    favorite_groups (user_id, group_id) {
        user_id -> Uuid,
        group_id -> Uuid,
    }
}

diesel::table! {
    favorite_producers (user_id, producer_id) {
        user_id -> Uuid,
        producer_id -> Uuid,
    }
}

diesel::table! {
    group_join_requests (id) {
        id -> Uuid,
        solicitor_user_id -> Uuid,
        target_group_id -> Uuid,
        created_at -> Timestamptz,
        accepted_at -> Nullable<Timestamptz>,
        declined_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    group_members (group_id, user_id) {
        group_id -> Uuid,
        user_id -> Uuid,
    }
}

diesel::table! {
    groups (id) {
        id -> Uuid,
        owner_user_id -> Uuid,
        location_id -> Uuid,
        is_personal_group -> Bool,
        name -> Text,
        description -> Text,
        modification -> Int2,
        visibility -> Int2,
    }
}

diesel::table! {
    internet_auths (producer_id) {
        producer_id -> Uuid,
        x25519_pub_key_bytes_base64 -> Text,
    }
}

diesel::table! {
    locations (id) {
        id -> Uuid,
        name -> Nullable<Text>,
        geo_lat -> Nullable<Float8>,
        geo_long -> Nullable<Float8>,
    }
}

diesel::table! {
    mqtt_broker_auths (producer_id) {
        producer_id -> Uuid,
        broker_host -> Text,
        broker_port -> Int2,
        username -> Text,
        password -> Text,
        topic -> Text,
    }
}

diesel::table! {
    packet_templates (id) {
        id -> Uuid,
        creator_user_id -> Nullable<Uuid>,
        name -> Text,
    }
}

diesel::table! {
    producers (id) {
        id -> Uuid,
        location_id -> Uuid,
        main_group_id -> Uuid,
        name -> Text,
        description -> Text,
        last_seen -> Timestamptz,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        forwarder_id -> Nullable<Uuid>,
        delivery -> Jsonb,
    }
}

diesel::table! {
    producers_for_groups (producer_id, group_id) {
        producer_id -> Uuid,
        group_id -> Uuid,
    }
}

diesel::table! {
    refresh_tokens (id) {
        id -> Uuid,
        user_id -> Uuid,
        claimed -> Bool,
        token_hash -> Text,
        expires_at -> Timestamptz,
        user_agent -> Text,
        ip_address -> Text,
    }
}

diesel::table! {
    template_field_mappings (template_id, field_id) {
        template_id -> Uuid,
        field_id -> Uuid,
        json_key -> Text,
    }
}

diesel::table! {
    template_fields (id) {
        id -> Uuid,
        name -> Text,
        repr_enum_value -> Int2,
        unit_enum_value -> Int2,
        unit_enum_other_payload -> Nullable<Text>,
    }
}

diesel::table! {
    users (id) {
        id -> Uuid,
        login -> Text,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        refreshed_token_at -> Timestamptz,
    }
}

diesel::joinable!(auth_methods -> users (user_id));
diesel::joinable!(commands -> producers (target_producer_id));
diesel::joinable!(data -> packet_templates (template_id));
diesel::joinable!(data -> producers (producer_id));
diesel::joinable!(favorite_groups -> groups (group_id));
diesel::joinable!(favorite_groups -> users (user_id));
diesel::joinable!(favorite_producers -> producers (producer_id));
diesel::joinable!(favorite_producers -> users (user_id));
diesel::joinable!(group_join_requests -> groups (target_group_id));
diesel::joinable!(group_join_requests -> users (solicitor_user_id));
diesel::joinable!(group_members -> groups (group_id));
diesel::joinable!(group_members -> users (user_id));
diesel::joinable!(groups -> locations (location_id));
diesel::joinable!(groups -> users (owner_user_id));
diesel::joinable!(internet_auths -> producers (producer_id));
diesel::joinable!(mqtt_broker_auths -> producers (producer_id));
diesel::joinable!(packet_templates -> users (creator_user_id));
diesel::joinable!(producers -> groups (main_group_id));
diesel::joinable!(producers -> locations (location_id));
diesel::joinable!(producers_for_groups -> groups (group_id));
diesel::joinable!(producers_for_groups -> producers (producer_id));
diesel::joinable!(refresh_tokens -> users (user_id));
diesel::joinable!(template_field_mappings -> packet_templates (template_id));
diesel::joinable!(template_field_mappings -> template_fields (field_id));

diesel::allow_tables_to_appear_in_same_query!(
    auth_methods,
    commands,
    data,
    favorite_groups,
    favorite_producers,
    group_join_requests,
    group_members,
    groups,
    internet_auths,
    locations,
    mqtt_broker_auths,
    packet_templates,
    producers,
    producers_for_groups,
    refresh_tokens,
    template_field_mappings,
    template_fields,
    users,
);
