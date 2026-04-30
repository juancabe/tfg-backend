INSERT INTO packet_templates (id, creator_user_id, name)
VALUES ('8f1ab69e-0fcb-4e18-9d3f-42f71d2f0001', NULL, 'randomfloat')
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
    '8f1ab69e-0fcb-4e18-9d3f-42f71d2f0002',
    '8f1ab69e-0fcb-4e18-9d3f-42f71d2f0001',
    'value',
    'value',
    1,
    5,
    NULL
)
ON CONFLICT (id) DO NOTHING;
