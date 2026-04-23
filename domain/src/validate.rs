use validator::ValidationError;

pub(crate) mod codes {
    pub const ASCII_DIGIT: &str = "ascii_digit";
    pub const SET_LIMIT: &str = "set_limit";
}

pub(crate) fn code(code: &str) -> Result<(), ValidationError> {
    if !code.chars().all(|c| c.is_ascii_digit()) {
        return Err(ValidationError::new(codes::ASCII_DIGIT));
    }

    Ok(())
}

pub(crate) fn validate_set_limit(
    com: &crate::transaction::SetLimit,
) -> Result<(), ValidationError> {
    if com.trans_limit > com.limit {
        return Err(ValidationError::new(codes::SET_LIMIT));
    }

    Ok(())
}
