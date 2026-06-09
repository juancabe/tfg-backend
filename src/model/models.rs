//! Diesel model structs representing database entities.
#![allow(missing_docs)]

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use super::schema::*;

// =========================================================================
//  ENUMS (Application Logic)
// =========================================================================

/// Authentication method kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum AuthKind {
    /// Email and password authentication.
    EmailPass = 1,
    /// OAuth-based authentication.
    OAuth = 2,
}

/// Data representation type for a template field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum FieldRepr {
    /// Floating-point number.
    Float = 1,
    /// Signed integer.
    Integer = 2,
    /// Unsigned integer.
    UnsignedInteger = 3,
    /// String value.
    String = 4,
    /// JSON object.
    Json = 5,
    /// Boolean value.
    Bool = 6,
}

/// Unit of measurement for a template field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum FieldUnit {
    /// Temperature in Celsius.
    TempCelsius = 1,
    /// Relative humidity percentage.
    HumidityPerc = 2,
    /// Voltage.
    Volts = 3,
    /// Unix epoch timestamp.
    UnixEpoch = 4,
    /// No unit.
    None = 5,
    /// Custom unit (uses a separate string payload column).
    Other = 6,
}

/// Status of a pending producer registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i16)]
pub enum PendingProducerRegistrationStatus {
    /// Registration is pending confirmation.
    Pending = 1,
    /// Registration has been confirmed.
    Confirmed = 2,
    /// Registration has expired.
    Expired = 3,
    /// Registration was cancelled.
    Cancelled = 4,
}

// =========================================================================
//  USERS & AUTH
// =========================================================================

/// Database entity for a user account.
#[derive(Debug, Queryable, Selectable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = users)]
pub struct User {
    pub id: Uuid,
    pub login: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub refreshed_token_at: DateTime<Utc>,
}

/// Insertable struct for creating a new user.
#[derive(Debug, Insertable, Clone)]
#[diesel(table_name = users)]
pub struct NewUser<'a> {
    pub login: &'a str,
}

/// Database entity for an email-based authentication method linked to a user.
#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(User))]
#[diesel(table_name = email_auth_methods)]
pub struct EmailAuthMethod {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub hashed_pass: String,
    pub created_at: DateTime<Utc>,
    pub verification_issued_at: DateTime<Utc>,
    pub is_verified: bool,
    pub changed_pass_at: DateTime<Utc>,
}

/// Insertable struct for creating a new email authentication method.
#[derive(Debug, Insertable)]
#[diesel(table_name = email_auth_methods)]
pub struct NewEmailAuthMethod<'a> {
    pub user_id: Uuid,
    pub email: &'a str,
    pub hashed_pass: &'a str,
}

/// Database entity for an email verification token.
#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(User))]
#[diesel(table_name = email_verifications)]
pub struct EmailVerification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
}

/// Insertable struct for creating a new email verification token.
#[derive(Debug, Insertable)]
#[diesel(table_name = email_verifications)]
pub struct NewEmailVerification<'a> {
    pub user_id: Uuid,
    pub email: &'a str,
    pub expires_at: DateTime<Utc>,
}

/// Database entity for a password reset request.
#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(User))]
#[diesel(table_name = password_resets)]
pub struct PasswordReset {
    pub id: Uuid,
    pub user_id: Uuid,
    pub email: String,
    // 6 number digit code
    pub code: String,
    // number of attempts while active
    pub attempts: i32,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub consumed_at: Option<DateTime<Utc>>,
}

/// Insertable struct for creating a new password reset request.
#[derive(Debug, Insertable)]
#[diesel(table_name = password_resets)]
pub struct NewPasswordReset<'a> {
    pub user_id: Uuid,
    pub email: &'a str,
    pub code: &'a str,
    pub expires_at: DateTime<Utc>,
}

/// Database entity for a refresh token used in JWT rotation.
#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(User))]
#[diesel(table_name = refresh_tokens)]
pub struct RefreshToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub claimed: bool,
    pub expires_at: DateTime<Utc>,
    pub version_n: i64,
}

/// Insertable struct for creating a new refresh token.
#[derive(Debug, Insertable, Clone, Copy)]
#[diesel(table_name = refresh_tokens)]
pub struct NewRefreshToken {
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
    // pub user_agent: String,
    // pub ip_address: String,
}

// =========================================================================
//  LOCATIONS
// =========================================================================

/// Database entity for a physical or logical location.
#[derive(Debug, Queryable, Selectable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = locations)]
pub struct Location {
    pub id: Uuid,
    pub name: Option<String>,
    pub geo_lat: Option<f64>,
    pub geo_long: Option<f64>,
}

/// Insertable struct for creating a new location.
#[derive(Debug, Insertable, Serialize, Deserialize)]
#[diesel(table_name = locations)]
pub struct NewLocation {
    pub name: Option<String>,
    pub geo_lat: Option<f64>,
    pub geo_long: Option<f64>,
}

// =========================================================================
//  GROUPS
// =========================================================================

/// Database entity for a group of producers owned by a user.
#[derive(
    Debug,
    Queryable,
    Selectable,
    Identifiable,
    Associations,
    Serialize,
    Deserialize,
    Hash,
    PartialEq,
    Eq,
    Clone,
)]
#[diesel(belongs_to(User, foreign_key = owner_user_id))]
#[diesel(belongs_to(Location))]
#[diesel(table_name = groups)]
pub struct Group {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub location_id: Uuid,
    pub name: String,
    pub description: String,
    pub is_public: bool,
}

/// Insertable struct for creating a new group.
#[derive(Debug, Insertable)]
#[diesel(table_name = groups)]
pub struct NewGroup {
    pub owner_user_id: Uuid,
    pub location_id: Uuid,
    pub name: String,
    pub description: String,
    pub is_public: bool,
}

/// Updateable fields for a group.
#[derive(Debug, AsChangeset)]
#[diesel(table_name = groups)]
pub struct GroupUpdate {
    pub description: Option<String>,
    pub location_id: Option<Uuid>,
    pub is_public: bool,
}

/// Insertable struct for creating a new favorite group association.
#[derive(Debug, Insertable, Serialize, Deserialize)]
#[diesel(table_name = favorite_groups)]
pub struct NewFavoriteGroup {
    pub user_id: Uuid,
    pub group_id: Uuid,
}

// =========================================================================
//  PRODUCERS
// =========================================================================

/// Database entity for a producer device.
#[derive(
    Debug,
    Queryable,
    Selectable,
    Identifiable,
    Associations,
    Serialize,
    Deserialize,
    Hash,
    PartialEq,
    Eq,
)]
#[diesel(belongs_to(Location))]
#[diesel(belongs_to(Group, foreign_key = group_id))]
#[diesel(table_name = producers)]
pub struct Producer {
    pub id: Uuid,
    pub location_id: Uuid,
    pub group_id: Uuid,
    pub name: String,
    pub ble_name: String,
    pub description: String,
    pub last_seen: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub forwarder_id: Option<Uuid>,
    pub lora_addr: Option<i32>,
    pub forward_lora: bool,
}

/// Insertable struct for creating a new producer.
#[derive(Debug, Insertable)]
#[diesel(table_name = producers)]
pub struct NewProducer {
    pub location_id: Uuid,
    pub group_id: Uuid,
    pub name: String,
    pub ble_name: String,
    pub description: String,
    pub forwarder_id: Option<Uuid>,
    pub lora_addr: Option<i32>,
    pub forward_lora: bool,
}

/// Updateable fields for a producer.
#[derive(Debug, AsChangeset)]
#[diesel(table_name = producers)]
pub struct ProducerUpdate {
    pub description: Option<String>,
    pub last_seen: Option<DateTime<Utc>>,
    pub forwarder_id: Option<Uuid>,
    pub lora_addr: Option<i32>,
    pub forward_lora: Option<bool>,
}

/// Database entity for internet authentication credentials of a producer.
#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(Producer))]
#[diesel(primary_key(producer_id))]
#[diesel(table_name = internet_auths)]
pub struct InternetAuth {
    pub producer_id: Uuid,
    pub x25519_pub_key_bytes_base64: String,
}

/// Insertable struct for creating new internet authentication credentials.
#[derive(Debug, Insertable, AsChangeset)]
#[diesel(table_name = internet_auths)]
pub struct NewInternetAuth {
    pub producer_id: Uuid,
    pub x25519_pub_key_bytes_base64: String,
}

/// Database entity for MQTT broker authentication credentials of a producer.
#[derive(
    Debug, Clone, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize,
)]
#[diesel(belongs_to(Producer))]
#[diesel(primary_key(producer_id))]
#[diesel(table_name = mqtt_broker_auths)]
pub struct MqttBrokerAuth {
    pub producer_id: Uuid,
    pub broker_host: String,
    pub broker_port: i16,
    pub username: String,
    pub password_hash: String,
    pub topic: String,
}

/// Insertable struct for creating new MQTT broker authentication credentials.
#[derive(Debug, Insertable, AsChangeset)]
#[diesel(table_name = mqtt_broker_auths)]
pub struct NewMqttBrokerAuth {
    pub producer_id: Uuid,
    pub broker_host: String,
    pub broker_port: i16,
    pub username: String,
    pub password_hash: String,
    pub topic: String,
}

/// Database entity for a pending producer registration request.
#[derive(Debug, Queryable, Selectable, Identifiable, Serialize, Deserialize)]
#[diesel(table_name = pending_producer_registrations)]
pub struct PendingProducerRegistration {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub group_id: Uuid,
    pub producer_id: Option<Uuid>,
    pub status: i16,
    pub expires_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub producer_name: String,
    pub ble_name: String,
    pub description: String,
    pub location_name: Option<String>,
    pub location_geo_lat: Option<f64>,
    pub location_geo_long: Option<f64>,
    pub forwarder_id: Option<Uuid>,
    pub forward_lora: bool,
    pub data_template_id: Option<Uuid>,
    pub mqtt_username: String,
    pub mqtt_password_hash: String,
    pub mqtt_topic: String,
    pub broker_host: String,
    pub broker_port: i16,
    pub x25519_pub_key_bytes_base64: Option<String>,
    pub lora_addr: Option<i32>,
}

/// Insertable struct for creating a new pending producer registration.
#[derive(Debug, Insertable)]
#[diesel(table_name = pending_producer_registrations)]
pub struct NewPendingProducerRegistration {
    pub owner_user_id: Uuid,
    pub group_id: Uuid,
    pub producer_id: Option<Uuid>,
    pub status: i16,
    pub expires_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub producer_name: String,
    pub ble_name: String,
    pub description: String,
    pub location_name: Option<String>,
    pub location_geo_lat: Option<f64>,
    pub location_geo_long: Option<f64>,
    pub forwarder_id: Option<Uuid>,
    pub forward_lora: bool,
    pub data_template_id: Option<Uuid>,
    pub mqtt_username: String,
    pub mqtt_password_hash: String,
    pub mqtt_topic: String,
    pub broker_host: String,
    pub broker_port: i16,
    pub x25519_pub_key_bytes_base64: Option<String>,
    pub lora_addr: Option<i32>,
}

/// Updateable fields for a pending producer registration.
#[derive(Debug, AsChangeset)]
#[diesel(table_name = pending_producer_registrations)]
pub struct PendingProducerRegistrationUpdate {
    pub producer_id: Option<Uuid>,
    pub status: Option<i16>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub x25519_pub_key_bytes_base64: Option<String>,
    pub lora_addr: Option<i32>,
}

/// Insertable struct for creating a new favorite producer association.
#[derive(Debug, Insertable, Serialize, Deserialize)]
#[diesel(table_name = favorite_producers)]
pub struct NewFavoriteProducer {
    pub user_id: Uuid,
    pub producer_id: Uuid,
}

// =========================================================================
//  TEMPLATES & DATA
// =========================================================================

/// Database entity for a data packet template.
#[derive(Debug, Queryable, Selectable, Identifiable, Associations, Serialize, Deserialize)]
#[diesel(belongs_to(User, foreign_key = creator_user_id))]
#[diesel(table_name = packet_templates)]
pub struct PacketTemplate {
    pub id: Uuid,
    pub creator_user_id: Option<Uuid>,
    pub name: String,
}

/// Insertable struct for creating a new packet template.
#[derive(Debug, Insertable)]
#[diesel(table_name = packet_templates)]
pub struct NewPacketTemplate {
    pub creator_user_id: Option<Uuid>,
    pub name: String,
}

/// Database entity for a field definition within a packet template.
#[derive(Debug, Queryable, Selectable, Identifiable, Serialize, Deserialize, Clone)]
#[diesel(table_name = template_fields)]
pub struct TemplateField {
    pub id: Uuid,
    pub template_id: Uuid,
    pub json_key: String,
    pub name: String,
    pub repr_enum_value: i16, // Map to FieldRepr
    pub unit_enum_value: i16, // Map to FieldUnit
    pub unit_enum_other_payload: Option<String>,
}

/// Insertable struct for creating a new template field.
#[derive(Debug, Insertable)]
#[diesel(table_name = template_fields)]
pub struct NewTemplateField {
    pub template_id: Uuid,
    pub json_key: String,
    pub name: String,
    pub repr_enum_value: i16,
    pub unit_enum_value: i16,
    pub unit_enum_other_payload: Option<String>,
}

/// Database entity for telemetry data received from a producer.
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

/// Insertable struct for creating a new data record.
#[derive(Debug, Insertable)]
#[diesel(table_name = data)]
pub struct NewData {
    pub producer_id: Uuid,
    pub template_id: Uuid,
    pub payload: JsonValue,
    // recv_at is default NOW()
}
