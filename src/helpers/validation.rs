//! Input validation helpers and field length constants.

use validator::ValidationError;

/// Minimum length for a user login.
pub const LOGIN_MIN_LEN: u64 = 3;
/// Maximum length for a user login.
pub const LOGIN_MAX_LEN: u64 = 32;
/// Minimum length for a password.
pub const PASSWORD_MIN_LEN: u64 = 8;
/// Maximum length for a password.
pub const PASSWORD_MAX_LEN: u64 = 128;
/// Minimum length for a name field.
pub const NAME_MIN_LEN: u64 = 1;
/// Maximum length for a name field.
pub const NAME_MAX_LEN: u64 = 32;
/// Maximum length for a description field.
pub const DESCRIPTION_MAX_LEN: u64 = 512;

/// Checks if a string contains MQTT topic meta-characters (`/`, `+`, `#`).
pub fn contains_mqtt_topic_meta(value: &str) -> bool {
    value.contains('/') || value.contains('+') || value.contains('#')
}

/// Validates that a firmware name is non-empty, within length limits, and contains no MQTT meta-characters.
pub fn validate_firmware_name(value: &str) -> Result<(), ValidationError> {
    if value.is_empty() || value.len() > NAME_MAX_LEN as usize {
        let mut error = ValidationError::new("length");
        error.message = Some(format!("must be 1..={NAME_MAX_LEN} characters").into());
        return Err(error);
    }

    if contains_mqtt_topic_meta(value) {
        let mut error = ValidationError::new("invalid_chars");
        error.message = Some("cannot include '/', '+' or '#'".into());
        return Err(error);
    }

    Ok(())
}
