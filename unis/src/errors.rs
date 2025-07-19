use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("命令数据验证错误：{0}")]
    CheckError(String),
    #[error("序列化错误")]
    EncodeError(#[from] bincode::error::EncodeError),
    #[error("反序列化错误")]
    DecodeError(#[from] bincode::error::DecodeError),
    #[error("发送错误")]
    SendError,
    #[error("接收错误")]
    RecvError,
    #[error("重复提交命令")]
    Duplicate,
    #[error("写入流存储错误：{0}")]
    WriteError(String),
    #[error("读取流存储错误：{0}")]
    ReadError(String),
}
