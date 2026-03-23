use validator::ValidationError;

#[allow(dead_code)]
mod codes {
    pub const REQUIRED: &str = "required";
    pub const MIN_LENGTH: &str = "min_length";
    pub const MAX_LENGTH: &str = "max_length";
    pub const EXACT_LENGTH: &str = "exact_length";
    pub const ASCII_DIGIT: &str = "ascii_digit";
}

pub(crate) fn code(code: &str) -> Result<(), ValidationError> {
    if !code.chars().all(|c| c.is_ascii_digit()) {
        return Err(ValidationError::new(codes::ASCII_DIGIT));
    }

    Ok(())
}
