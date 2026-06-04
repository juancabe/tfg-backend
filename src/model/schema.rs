// @generated automatically by Diesel CLI.

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
    email_auth_methods (id) {
        id -> Uuid,
        user_id -> Uuid,
        email -> Text,
        hashed_pass -> Text,
        created_at -> Timestamptz,
        verification_issued_at -> Timestamptz,
        is_verified -> Bool,
        changed_pass_at -> Timestamptz,
    }
}

diesel::table! {
    email_verifications (id) {
        id -> Uuid,
        user_id -> Uuid,
        email -> Text,
        created_at -> Timestamptz,
        expires_at -> Timestamptz,
        consumed_at -> Nullable<Timestamptz>,
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
    groups (id) {
        id -> Uuid,
        owner_user_id -> Uuid,
        location_id -> Uuid,
        name -> Text,
        description -> Text,
        is_public -> Bool,
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
        password_hash -> Text,
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
    password_resets (id) {
        id -> Uuid,
        user_id -> Uuid,
        email -> Text,
        code -> Text,
        attempts -> Int4,
        created_at -> Timestamptz,
        expires_at -> Timestamptz,
        consumed_at -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    pending_producer_registrations (id) {
        id -> Uuid,
        owner_user_id -> Uuid,
        group_id -> Uuid,
        producer_id -> Nullable<Uuid>,
        status -> Int2,
        expires_at -> Timestamptz,
        confirmed_at -> Nullable<Timestamptz>,
        cancelled_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        producer_name -> Text,
        ble_name -> Text,
        description -> Text,
        location_name -> Nullable<Text>,
        location_geo_lat -> Nullable<Float8>,
        location_geo_long -> Nullable<Float8>,
        forwarder_id -> Nullable<Uuid>,
        data_template_id -> Nullable<Uuid>,
        mqtt_username -> Text,
        mqtt_password_hash -> Text,
        mqtt_topic -> Text,
        broker_host -> Text,
        broker_port -> Int2,
        x25519_pub_key_bytes_base64 -> Nullable<Text>,
        lora_addr -> Nullable<Int4>,
        forward_lora -> Bool,
    }
}

diesel::table! {
    producers (id) {
        id -> Uuid,
        location_id -> Uuid,
        group_id -> Uuid,
        name -> Text,
        ble_name -> Text,
        description -> Text,
        last_seen -> Timestamptz,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        forwarder_id -> Nullable<Uuid>,
        lora_addr -> Nullable<Int4>,
        forward_lora -> Bool,
    }
}

diesel::table! {
    refresh_tokens (id) {
        id -> Uuid,
        user_id -> Uuid,
        claimed -> Bool,
        expires_at -> Timestamptz,
        version_n -> Int8,
    }
}

diesel::table! {
    template_fields (id) {
        id -> Uuid,
        template_id -> Uuid,
        name -> Text,
        json_key -> Text,
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

diesel::joinable!(data -> packet_templates (template_id));
diesel::joinable!(data -> producers (producer_id));
diesel::joinable!(email_auth_methods -> users (user_id));
diesel::joinable!(email_verifications -> users (user_id));
diesel::joinable!(favorite_groups -> groups (group_id));
diesel::joinable!(favorite_groups -> users (user_id));
diesel::joinable!(favorite_producers -> producers (producer_id));
diesel::joinable!(favorite_producers -> users (user_id));
diesel::joinable!(groups -> locations (location_id));
diesel::joinable!(groups -> users (owner_user_id));
diesel::joinable!(internet_auths -> producers (producer_id));
diesel::joinable!(mqtt_broker_auths -> producers (producer_id));
diesel::joinable!(packet_templates -> users (creator_user_id));
diesel::joinable!(password_resets -> users (user_id));
diesel::joinable!(pending_producer_registrations -> groups (group_id));
diesel::joinable!(pending_producer_registrations -> packet_templates (data_template_id));
diesel::joinable!(pending_producer_registrations -> users (owner_user_id));
diesel::joinable!(producers -> groups (group_id));
diesel::joinable!(producers -> locations (location_id));
diesel::joinable!(refresh_tokens -> users (user_id));
diesel::joinable!(template_fields -> packet_templates (template_id));

diesel::allow_tables_to_appear_in_same_query!(
    data,
    email_auth_methods,
    email_verifications,
    favorite_groups,
    favorite_producers,
    groups,
    internet_auths,
    locations,
    mqtt_broker_auths,
    packet_templates,
    password_resets,
    pending_producer_registrations,
    producers,
    refresh_tokens,
    template_fields,
    users,
);
