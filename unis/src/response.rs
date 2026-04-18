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
    /// 被冲突的新命令取代
    Conflict = 4,
    /// 命令已执行成功
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
            4 => UniResponse::Conflict,
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
