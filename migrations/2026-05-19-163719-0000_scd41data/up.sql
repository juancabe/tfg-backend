-- Your SQL goes here
INSERT INTO packet_templates (id, creator_user_id, name)
VALUES ('992be127-edf8-443e-a301-cd17121672d7', NULL, 'scd41data')
ON CONFLICT (id) DO NOTHING;

INSERT INTO template_fields (
    id,
    template_id,
    name,
    json_key,
    repr_enum_value,
    unit_enum_value,
    unit_enum_other_payload
)
VALUES (
    '3c9d7bcd-7a5f-4285-b182-c1c5807f7bd3',
    '992be127-edf8-443e-a301-cd17121672d7',
    'co2',
    'co2',
    1,
    5,
    'CO2 PPM'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO template_fields (
    id,
    template_id,
    name,
    json_key,
    repr_enum_value,
    unit_enum_value,
    unit_enum_other_payload
)
VALUES (
    '1e2e8cd0-240b-42da-8f79-87753ca8d689',
    '992be127-edf8-443e-a301-cd17121672d7',
    'temperature',
    'temperature',
    1,
    1,
    NULL
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO template_fields (
    id,
    template_id,
    name,
    json_key,
    repr_enum_value,
    unit_enum_value,
    unit_enum_other_payload
)
VALUES (
    '79edb8d4-f985-426d-a4f2-e27e71cb6872',
    '992be127-edf8-443e-a301-cd17121672d7',
    'humidity',
    'humidity',
    1,
    2,
    NULL
)
ON CONFLICT (id) DO NOTHING;
