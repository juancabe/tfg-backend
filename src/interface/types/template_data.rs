use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::model::models;

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct TemplateFieldView {
    pub id: Uuid,
    pub template_id: Uuid,
    pub json_key: String,
    pub name: String,
    pub repr_enum_value: i16,
    pub repr_name: String,
    pub unit_enum_value: i16,
    pub unit_name: String,
    pub unit_enum_other_payload: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct PacketTemplateView {
    pub id: Uuid,
    pub creator_user_id: Option<Uuid>,
    pub name: String,
    pub fields: Vec<TemplateFieldView>,
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct DataSampleView {
    pub id: Uuid,
    pub producer_id: Uuid,
    pub template_id: Uuid,
    pub recv_at: DateTime<Utc>,
    #[schema(value_type = Object)]
    pub payload: JsonValue,
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct ProducerDataQuery {
    pub template_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, utoipa::ToSchema)]
pub struct ProducerDataView {
    pub producer_id: Uuid,
    pub samples: Vec<DataSampleView>,
    pub templates: Vec<PacketTemplateView>,
}

fn repr_name_from_i16(v: i16) -> String {
    match v {
        x if x == models::FieldRepr::Float as i16 => "float",
        x if x == models::FieldRepr::Integer as i16 => "integer",
        x if x == models::FieldRepr::UnsignedInteger as i16 => "unsigned_integer",
        x if x == models::FieldRepr::String as i16 => "string",
        x if x == models::FieldRepr::Json as i16 => "json",
        x if x == models::FieldRepr::Bool as i16 => "bool",
        _ => "unknown",
    }
    .to_string()
}

fn unit_name_from_i16(v: i16) -> String {
    match v {
        x if x == models::FieldUnit::TempCelsius as i16 => "temp_celsius",
        x if x == models::FieldUnit::HumidityPerc as i16 => "humidity_percent",
        x if x == models::FieldUnit::Volts as i16 => "volts",
        x if x == models::FieldUnit::UnixEpoch as i16 => "unix_epoch",
        x if x == models::FieldUnit::None as i16 => "none",
        x if x == models::FieldUnit::Other as i16 => "other",
        _ => "unknown",
    }
    .to_string()
}

impl From<models::TemplateField> for TemplateFieldView {
    fn from(value: models::TemplateField) -> Self {
        Self {
            id: value.id,
            template_id: value.template_id,
            json_key: value.json_key,
            name: value.name,
            repr_enum_value: value.repr_enum_value,
            repr_name: repr_name_from_i16(value.repr_enum_value),
            unit_enum_value: value.unit_enum_value,
            unit_name: unit_name_from_i16(value.unit_enum_value),
            unit_enum_other_payload: value.unit_enum_other_payload,
        }
    }
}

impl From<(models::PacketTemplate, Vec<models::TemplateField>)> for PacketTemplateView {
    fn from(value: (models::PacketTemplate, Vec<models::TemplateField>)) -> Self {
        let (template, fields) = value;
        Self {
            id: template.id,
            creator_user_id: template.creator_user_id,
            name: template.name,
            fields: fields.into_iter().map(TemplateFieldView::from).collect(),
        }
    }
}

impl From<models::Data> for DataSampleView {
    fn from(value: models::Data) -> Self {
        Self {
            id: value.id,
            producer_id: value.producer_id,
            template_id: value.template_id,
            recv_at: value.recv_at,
            payload: value.payload,
        }
    }
}
