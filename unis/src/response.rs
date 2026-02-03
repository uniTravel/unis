use axum::{http::StatusCode, response::IntoResponse};

/// 命令处理结果枚举
#[repr(u8)]
#[derive(Debug, PartialEq)]
pub enum UniResponse {
    /// 成功
    Success = 0,
    /// 命令数据验证错误
    ValidateError = 1,
    /// 身份验证错误
    AuthError = 2,
    /// 请求超时
    Timeout = 3,
    /// 命令重复提交
    Duplicate = 11,
    /// 命令无法应用到聚合
    CheckError = 12,
    /// 序列化错误
    CodeError = 13,
    /// 消息处理错误
    MsgError = 14,
    /// 写入事件流错误
    WriteError = 15,
    /// 读取事件流错误
    ReadError = 16,
    /// 写入命令流错误
    SendError = 17,
}

impl UniResponse {
    /// 反序列化
    pub fn from_bytes(res_data: &[u8]) -> Self {
        match res_data[0] {
            0 => UniResponse::Success,
            1 => UniResponse::ValidateError,
            2 => UniResponse::AuthError,
            3 => UniResponse::Timeout,
            11 => UniResponse::Duplicate,
            12 => UniResponse::CheckError,
            13 => UniResponse::CodeError,
            14 => UniResponse::MsgError,
            15 => UniResponse::WriteError,
            16 => UniResponse::ReadError,
            17 => UniResponse::SendError,
            _ => panic!("转换命令处理结果枚举失败"),
        }
    }
    /// 序列化
    pub fn to_bytes(self) -> [u8; 1] {
        [self as u8]
    }
}

impl std::fmt::Display for UniResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UniResponse::Success => write!(f, "成功"),
            UniResponse::ValidateError => write!(f, "命令数据验证错误"),
            UniResponse::AuthError => write!(f, "身份验证错误"),
            UniResponse::Timeout => write!(f, "请求超时"),
            UniResponse::Duplicate => write!(f, "命令重复提交"),
            UniResponse::CheckError => write!(f, "命令无法应用到聚合"),
            UniResponse::CodeError => write!(f, "序列化错误"),
            UniResponse::MsgError => write!(f, "消息处理错误"),
            UniResponse::WriteError => write!(f, "写入事件流错误"),
            UniResponse::ReadError => write!(f, "读取事件流错误"),
            UniResponse::SendError => write!(f, "写入命令流错误"),
        }
    }
}

impl IntoResponse for UniResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            UniResponse::Success => StatusCode::OK.into_response(),
            UniResponse::ValidateError => {
                (StatusCode::BAD_REQUEST, "命令数据验证错误").into_response()
            }
            UniResponse::AuthError => (StatusCode::UNAUTHORIZED, "身份验证错误").into_response(),
            UniResponse::Timeout => (StatusCode::REQUEST_TIMEOUT, "请求超时").into_response(),
            UniResponse::Duplicate => {
                (StatusCode::INTERNAL_SERVER_ERROR, "命令重复提交").into_response()
            }
            UniResponse::CheckError => {
                (StatusCode::BAD_REQUEST, "命令无法应用到聚合").into_response()
            }
            UniResponse::CodeError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "序列化错误").into_response()
            }
            UniResponse::MsgError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "消息处理错误").into_response()
            }
            UniResponse::WriteError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "写入事件流错误").into_response()
            }
            UniResponse::ReadError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "读取事件流错误").into_response()
            }
            UniResponse::SendError => {
                (StatusCode::INTERNAL_SERVER_ERROR, "写入命令流错误").into_response()
            }
        }
    }
}
