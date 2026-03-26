use validator::ValidationError;

pub(crate) mod codes {
    pub const ASCII_DIGIT: &str = "ascii_digit";
}

pub(crate) fn code(code: &str) -> Result<(), ValidationError> {
    if !code.chars().all(|c| c.is_ascii_digit()) {
        return Err(ValidationError::new(codes::ASCII_DIGIT));
    }

    Ok(())
}
