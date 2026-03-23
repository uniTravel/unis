use validator::ValidationError;

#[allow(dead_code)]
mod codes {
    pub const REQUIRED: &str = "required";
    pub const MIN_LENGTH: &str = "min_length";
    pub const MAX_LENGTH: &str = "max_length";
    pub const EXACT_LENGTH: &str = "exact_length";
    pub const ASCII_DIGIT: &str = "ascii_digit";
}

// trait Cn {
//     fn cn(&self) -> String;
// }

// impl Cn for ValidationError {
//     fn cn(&self) -> String {
//         todo!()
//     }
// }

// fn handle_errors(errors: validator::ValidationErrors) -> Vec<String> {
//     let mut friendly_errors = Vec::new();

//     for (field, field_errors) in errors.field_errors() {
//         for error in field_errors {
//             let message = match error.code.as_ref() {
//                 codes::USERNAME_REQUIRED => "用户名是必填项".to_string(),
//                 codes::USERNAME_MIN_LENGTH => {
//                     let min = error.params.get("min").unwrap();
//                     format!("用户名长度不能少于 {}", min)
//                 }
//                 codes::USERNAME_MAX_LENGTH => {
//                     let max = error.params.get("max").unwrap();
//                     format!("用户名长度不能超过 {}", max)
//                 }
//                 codes::USERNAME_INVALID_CHARS => "用户名只能包含字母和数字".to_string(),
//                 codes::PASSWORD_WEAK => "密码强度不足".to_string(),
//                 codes::EMAIL_DOMAIN_BLOCKED => "该邮箱域名已被禁止".to_string(),
//                 _ => error.message.clone().unwrap_or_else(|| "验证失败".to_string()),
//             };

//             friendly_errors.push(format!("{}: {}", field, message));
//         }
//     }

//     friendly_errors
// }

pub(crate) fn code(code: &str) -> Result<(), ValidationError> {
    // let len = code.chars().count();
    // if len != 6 {
    //     let mut err = ValidationError::new(codes::EXACT_LENGTH);
    //     // err.message = Some("长度错误".into());
    //     err.add_param("exact".into(), &6);
    //     return Err(err);
    // }

    if !code.chars().all(|c| c.is_ascii_digit()) {
        return Err(ValidationError::new(codes::ASCII_DIGIT));
    }

    Ok(())
}
