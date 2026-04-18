use axum::http::StatusCode;
use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{self, Write},
    sync::LazyLock,
};
use validator::{ValidationError, ValidationErrors, ValidationErrorsKind};

use crate::UniResponse;
#[cfg(any(test, feature = "test-utils"))]
use crate::domain::Command;
#[cfg(any(test, feature = "test-utils"))]
use serde::de::DeserializeOwned;
#[cfg(any(test, feature = "test-utils"))]
use validator::Validate;

static CONFIG: LazyLock<config::Config> = LazyLock::new(|| crate::config::i18n_config());

static VALIDATION: LazyLock<HashMap<String, HashMap<String, String>>> =
    LazyLock::new(|| crate::config::load_named_setting(&CONFIG, "validation"));

static RESPONSE: LazyLock<HashMap<String, HashMap<String, String>>> =
    LazyLock::new(|| crate::config::load_named_setting(&CONFIG, "response"));

#[doc(hidden)]
#[cfg(any(test, feature = "test-utils"))]
pub fn validate<T>(com: &T, lang: &str) -> Result<(), String>
where
    T: Command + Validate + DeserializeOwned,
{
    if let Err(e) = com.validate() {
        let mut result = String::new();
        return match validation(&mut result, &e, lang) {
            Ok(()) => Err(result),
            Err(_) => Err(e.to_string()),
        };
    }
    Ok(())
}

pub(crate) fn validation(result: &mut String, e: &ValidationErrors, lang: &str) -> fmt::Result {
    let errors = e.errors();
    for (idx, (path, errs)) in errors.iter().enumerate() {
        display_errors(result, errs, path, lang)?;
        if idx + 1 < errors.len() {
            result.push('\n');
        }
    }
    Ok(())
}

pub(crate) fn response(res: UniResponse, lang: &str) -> (StatusCode, String) {
    let (status, code) = match res {
        UniResponse::ValidateError => (StatusCode::BAD_REQUEST, "validate"),
        UniResponse::AuthError => (StatusCode::UNAUTHORIZED, "auth"),
        UniResponse::Timeout => (StatusCode::REQUEST_TIMEOUT, "timeout"),
        UniResponse::Conflict => (StatusCode::CONFLICT, "conflict"),
        UniResponse::CheckError => (StatusCode::INTERNAL_SERVER_ERROR, "check"),
        UniResponse::CodeError => (StatusCode::INTERNAL_SERVER_ERROR, "code"),
        UniResponse::MsgError => (StatusCode::INTERNAL_SERVER_ERROR, "msg"),
        UniResponse::WriteError => (StatusCode::INTERNAL_SERVER_ERROR, "write"),
        UniResponse::ReadError => (StatusCode::INTERNAL_SERVER_ERROR, "read"),
        UniResponse::SendError => (StatusCode::INTERNAL_SERVER_ERROR, "send"),
        UniResponse::Success => (StatusCode::OK, "_"),
        UniResponse::Duplicate => (StatusCode::ACCEPTED, "_"),
    };
    match RESPONSE
        .get(lang)
        .or(RESPONSE.get("zh"))
        .and_then(|l| l.get(code))
    {
        Some(r) => (status, r.to_string()),
        None => (status, code.to_string()),
    }
}

fn display_errors(
    result: &mut String,
    errs: &ValidationErrorsKind,
    path: &str,
    lang: &str,
) -> fmt::Result {
    fn display_struct(
        result: &mut String,
        errs: &ValidationErrors,
        path: &str,
        lang: &str,
    ) -> fmt::Result {
        let mut full_path = String::new();
        full_path.push_str(path);
        full_path.push('.');
        let base_len = full_path.len();
        for (path, err) in errs.errors() {
            full_path.push_str(path);
            display_errors(result, err, &full_path, lang)?;
            result.push('\n');
            full_path.truncate(base_len);
        }
        Ok(())
    }

    match errs {
        ValidationErrorsKind::Field(errs) => {
            result.push_str(path);
            result.push_str(": ");
            let len = errs.len();
            for (idx, err) in errs.iter().enumerate() {
                if idx + 1 == len {
                    result.push_str(&render_template(err, lang));
                } else {
                    result.push_str(&render_template(err, lang));
                    result.push_str(", ");
                }
            }
        }
        ValidationErrorsKind::Struct(errs) => display_struct(result, errs, path, lang)?,
        ValidationErrorsKind::List(errs) => {
            let mut full_path = String::new();
            full_path.push_str(path);
            let base_len = full_path.len();
            for (idx, err) in errs.iter() {
                write!(&mut full_path, "[{}]", idx)?;
                display_struct(result, err, &full_path, lang)?;
                full_path.truncate(base_len);
            }
        }
    }

    Ok(())
}

fn render_template(err: &ValidationError, lang: &str) -> Cow<'static, str> {
    let template = match VALIDATION
        .get(lang)
        .or(VALIDATION.get("zh"))
        .and_then(|l| l.get(err.code.as_ref()))
    {
        Some(template) => template,
        None => return err.code.clone(),
    };

    let mut result = String::with_capacity(template.len() + 64);
    let mut pos = 0;

    while let Some(open_brace) = template[pos..].find('{') {
        let start = pos + open_brace;
        if let Some(close_brace) = template[start + 1..].find('}') {
            let placeholder_end = start + 1 + close_brace;
            let placeholder = &template[start + 1..placeholder_end];
            result.push_str(&template[pos..start]);

            match err.params.get(placeholder) {
                Some(value) => result.push_str(&value.to_string()),
                None => result.push_str(&template[start..=placeholder_end]),
            }

            pos = placeholder_end + 1;
        }
    }

    if pos == 0 {
        return Cow::Borrowed(template);
    }

    if pos < template.len() {
        result.push_str(&template[pos..]);
    }

    Cow::Owned(result)
}
