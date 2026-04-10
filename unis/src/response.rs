use crate::{UniResponse, i18n};
use axum::{http::StatusCode, response::IntoResponse};

impl std::fmt::Display for UniResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UniResponse::Success => write!(f, "成功"),
            UniResponse::ValidateError => write!(f, "命令数据验证错误"),
            UniResponse::AuthError => write!(f, "身份验证错误"),
            UniResponse::Timeout => write!(f, "请求超时"),
            UniResponse::Conflict => write!(f, "被冲突的新命令取代"),
            UniResponse::Duplicate => write!(f, "命令已执行成功"),
            UniResponse::CheckError => write!(f, "命令无法应用到聚合"),
            UniResponse::CodeError => write!(f, "序列化错误"),
            UniResponse::MsgError => write!(f, "消息处理错误"),
            UniResponse::WriteError => write!(f, "写入事件流错误"),
            UniResponse::ReadError => write!(f, "读取事件流错误"),
            UniResponse::SendError => write!(f, "写入命令流错误"),
        }
    }
}

/// 国际化的命令处理结果
pub struct I18nResponse(pub UniResponse, pub String, pub uuid::Uuid);

impl IntoResponse for I18nResponse {
    fn into_response(self) -> axum::response::Response {
        match self.0 {
            UniResponse::Success => (StatusCode::OK, self.2.to_string()).into_response(),
            UniResponse::ValidateError => {
                (StatusCode::BAD_REQUEST, i18n::response("validate", &self.1)).into_response()
            }
            UniResponse::AuthError => {
                (StatusCode::UNAUTHORIZED, i18n::response("auth", &self.1)).into_response()
            }
            UniResponse::Timeout => (
                StatusCode::REQUEST_TIMEOUT,
                i18n::response("timeout", &self.1),
            )
                .into_response(),
            UniResponse::Conflict => {
                (StatusCode::CONFLICT, i18n::response("conflict", &self.1)).into_response()
            }
            UniResponse::Duplicate => {
                (StatusCode::OK, i18n::response("duplicate", &self.1)).into_response()
            }
            UniResponse::CheckError => {
                (StatusCode::BAD_REQUEST, i18n::response("check", &self.1)).into_response()
            }
            UniResponse::CodeError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                i18n::response("code", &self.1),
            )
                .into_response(),
            UniResponse::MsgError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                i18n::response("msg", &self.1),
            )
                .into_response(),
            UniResponse::WriteError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                i18n::response("write", &self.1),
            )
                .into_response(),
            UniResponse::ReadError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                i18n::response("read", &self.1),
            )
                .into_response(),
            UniResponse::SendError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                i18n::response("send", &self.1),
            )
                .into_response(),
        }
    }
}
