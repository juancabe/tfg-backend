use validator::ValidationError;

pub const LOGIN_MIN_LEN: u64 = 3;
pub const LOGIN_MAX_LEN: u64 = 32;
pub const PASSWORD_MIN_LEN: u64 = 8;
pub const PASSWORD_MAX_LEN: u64 = 128;
pub const NAME_MIN_LEN: u64 = 1;
pub const NAME_MAX_LEN: u64 = 32;
pub const DESCRIPTION_MAX_LEN: u64 = 512;

pub fn contains_mqtt_topic_meta(value: &str) -> bool {
    value.contains('/') || value.contains('+') || value.contains('#')
}

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
