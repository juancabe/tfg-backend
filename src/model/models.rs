use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::schema::*;

// =========================================================================
//  ENUMS (Application Logic)
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum AuthKind {
    EmailPass = 1,
    OAuth = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum GroupModification {
    AllCanPublish = 1,
    MembersCanPublish = 2,
    OwnerCanPublish = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum GroupVisibility {
    AllCanView = 1,
    MembersCanView = 2,
    OwnerCanView = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum CommandStatus {
    Sent = 1,
    Received = 2,
    ExecSuccess = 3,
    ExecFailure = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum CommandKind {
    Reboot = 1,
    UpdateFirmware = 2,
    SetWifi = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum FieldRepr {
    Float = 1,
    Integer = 2,
    UnsignedInteger = 3,
    String = 4,
    Json = 5,
    Bool = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum FieldUnit {
    TempCelsius = 1,
    HumidityPerc = 2,
    Volts = 3,
    UnixEpoch = 4,
    None = 5,
    Other = 6, // Uses the separate string payload column
}

// =========================================================================
//  USERS & AUTH
// =========================================================================

#[derive(Debug, Queryable, Selectable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = users)]
pub struct User {
    pub id: Uuid,
    pub login: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub refreshed_token_at: DateTime<Utc>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = users)]
pub struct NewUser<'a> {
    pub login: &'a str,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(User))]
#[diesel(table_name = auth_methods)]
pub struct AuthMethod {
    pub id: Uuid,
    pub user_id: Uuid,
    pub is_primary_auth: bool,
    pub kind: i16, // Map to AuthKind
    pub auth_data: JsonValue,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = auth_methods)]
pub struct NewAuthMethod {
    pub user_id: Uuid,
    pub is_primary_auth: bool,
    pub kind: i16,
    pub auth_data: JsonValue,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(User))]
#[diesel(table_name = refresh_tokens)]
pub struct RefreshToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub claimed: bool,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub user_agent: String,
    pub ip_address: String,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = refresh_tokens)]
pub struct NewRefreshToken {
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
    pub user_agent: String,
    pub ip_address: String,
}

// =========================================================================
//  LOCATIONS
// =========================================================================

#[derive(Debug, Queryable, Selectable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = locations)]
pub struct Location {
    pub id: Uuid,
    pub name: Option<String>,
    pub geo_lat: Option<f64>,
    pub geo_long: Option<f64>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = locations)]
pub struct NewLocation {
    pub name: Option<String>,
    pub geo_lat: Option<f64>,
    pub geo_long: Option<f64>,
}

// =========================================================================
//  GROUPS
// =========================================================================

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(User, foreign_key = owner_user_id))]
#[diesel(belongs_to(Location))]
#[diesel(table_name = groups)]
pub struct Group {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub location_id: Uuid,
    pub is_personal_group: bool,
    pub name: String,
    pub description: String,
    pub modification: i16, // Map to GroupModification
    pub visibility: i16,   // Map to GroupVisibility
}

#[derive(Debug, Insertable)]
#[diesel(table_name = groups)]
pub struct NewGroup {
    pub owner_user_id: Uuid,
    pub location_id: Uuid,
    pub is_personal_group: bool,
    pub name: String,
    pub description: String,
    pub modification: i16,
    pub visibility: i16,
}

#[derive(Debug, AsChangeset)]
#[diesel(table_name = groups)]
pub struct GroupUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub location_id: Option<Uuid>,
    pub modification: Option<i16>,
    pub visibility: Option<i16>,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(User, foreign_key = solicitor_user_id))]
#[diesel(belongs_to(Group, foreign_key = target_group_id))]
#[diesel(table_name = group_join_requests)]
pub struct GroupJoinRequest {
    pub id: Uuid,
    pub solicitor_user_id: Uuid,
    pub target_group_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub declined_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = group_join_requests)]
pub struct NewGroupJoinRequest {
    pub solicitor_user_id: Uuid,
    pub target_group_id: Uuid,
}

#[derive(Debug, Insertable, Serialize, Deserialize)]
#[diesel(table_name = group_members)]
pub struct NewGroupMember {
    pub group_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Debug, Insertable, Serialize, Deserialize)]
#[diesel(table_name = favorite_groups)]
pub struct NewFavoriteGroup {
    pub user_id: Uuid,
    pub group_id: Uuid,
}

// =========================================================================
//  PRODUCERS
// =========================================================================

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(Location))]
#[diesel(belongs_to(Group, foreign_key = main_group_id))]
#[diesel(table_name = producers)]
pub struct Producer {
    pub id: Uuid,
    pub location_id: Uuid,
    pub main_group_id: Uuid,
    pub name: String,
    pub description: String,
    pub last_seen: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub forwarder_id: Option<Uuid>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = producers)]
pub struct NewProducer {
    pub location_id: Uuid,
    pub main_group_id: Uuid,
    pub name: String,
    pub description: String,
    pub forwarder_id: Option<Uuid>,
}

#[derive(Debug, AsChangeset)]
#[diesel(table_name = producers)]
pub struct ProducerUpdate {
    pub name: Option<String>,
    pub description: Option<String>,
    pub last_seen: Option<DateTime<Utc>>,
    pub forwarder_id: Option<Uuid>,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(Producer))]
#[diesel(primary_key(producer_id))]
#[diesel(table_name = internet_auths)]
pub struct InternetAuth {
    pub producer_id: Uuid,
    pub x25519_pub_key_bytes_base64: String,
}

#[derive(Debug, Insertable, AsChangeset)]
#[diesel(table_name = internet_auths)]
pub struct NewInternetAuth {
    pub producer_id: Uuid,
    pub x25519_pub_key_bytes_base64: String,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(Producer))]
#[diesel(primary_key(producer_id))]
#[diesel(table_name = mqtt_broker_auths)]
pub struct MqttBrokerAuth {
    pub producer_id: Uuid,
    pub broker_host: String,
    pub broker_port: i16,
    pub username: String,
    pub password: String,
    pub topic: String,
}

#[derive(Debug, Insertable, AsChangeset)]
#[diesel(table_name = mqtt_broker_auths)]
pub struct NewMqttBrokerAuth {
    pub producer_id: Uuid,
    pub broker_host: String,
    pub broker_port: i16,
    pub username: String,
    pub password: String,
    pub topic: String,
}

#[derive(Debug, Insertable, Serialize, Deserialize)]
#[diesel(table_name = producers_for_groups)]
pub struct NewProducerForGroup {
    pub producer_id: Uuid,
    pub group_id: Uuid,
}

#[derive(Debug, Insertable, Serialize, Deserialize)]
#[diesel(table_name = favorite_producers)]
pub struct NewFavoriteProducer {
    pub user_id: Uuid,
    pub producer_id: Uuid,
}

// =========================================================================
//  COMMANDS
// =========================================================================

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(Producer, foreign_key = target_producer_id))]
#[diesel(table_name = commands)]
pub struct Command {
    pub id: Uuid,
    pub target_producer_id: Uuid,
    pub status: i16, // Map to CommandStatus
    pub kind: i16,   // Map to CommandKind
    pub created_at: DateTime<Utc>,
    pub payload: JsonValue,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = commands)]
pub struct NewCommand {
    pub target_producer_id: Uuid,
    pub status: i16,
    pub kind: i16,
    pub payload: JsonValue,
}

// =========================================================================
//  TEMPLATES & DATA
// =========================================================================

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(User, foreign_key = creator_user_id))]
#[diesel(table_name = packet_templates)]
pub struct PacketTemplate {
    pub id: Uuid,
    pub creator_user_id: Option<Uuid>,
    pub name: String,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = packet_templates)]
pub struct NewPacketTemplate {
    pub creator_user_id: Option<Uuid>,
    pub name: String,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = template_fields)]
pub struct TemplateField {
    pub id: Uuid,
    pub name: String,
    pub repr_enum_value: i16, // Map to FieldRepr
    pub unit_enum_value: i16, // Map to FieldUnit
    pub unit_enum_other_payload: Option<String>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = template_fields)]
pub struct NewTemplateField {
    pub name: String,
    pub repr_enum_value: i16,
    pub unit_enum_value: i16,
    pub unit_enum_other_payload: Option<String>,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(PacketTemplate, foreign_key = template_id))]
#[diesel(belongs_to(TemplateField, foreign_key = field_id))]
#[diesel(primary_key(template_id, field_id))]
#[diesel(table_name = template_field_mappings)]
pub struct TemplateFieldMapping {
    pub template_id: Uuid,
    pub field_id: Uuid,
    pub json_key: String,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = template_field_mappings)]
pub struct NewTemplateFieldMapping {
    pub template_id: Uuid,
    pub field_id: Uuid,
    pub json_key: String,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(Producer))]
#[diesel(belongs_to(PacketTemplate, foreign_key = template_id))]
#[diesel(table_name = data)]
pub struct Data {
    pub id: Uuid,
    pub producer_id: Uuid,
    pub template_id: Uuid,
    pub recv_at: DateTime<Utc>,
    pub payload: JsonValue,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = data)]
pub struct NewData {
    pub producer_id: Uuid,
    pub template_id: Uuid,
    pub payload: JsonValue,
    // recv_at is default NOW()
}
