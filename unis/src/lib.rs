//! # **unis** 核心库
//!
//!

#![warn(missing_docs)]

pub mod aggregator;
pub mod config;
pub mod domain;
pub mod errors;
pub mod pool;
#[cfg(feature = "test-utils")]
pub mod test_utils;

use bincode::config::Configuration;
use tokio::sync::OwnedSemaphorePermit;
use uuid::Uuid;

const EMPTY_BYTES: &[u8] = &[];

/// bincode 标准配置
pub const BINCODE_CONFIG: Configuration = bincode::config::standard();

/// 命令消息结构
pub struct Com {
    /// 聚合 Id
    pub agg_id: Uuid,
    /// 命令 Id
    pub com_id: Uuid,
    /// 命令数据
    pub com_data: Vec<u8>,
    /// 追踪跨度
    pub span: tracing::Span,
}

/// 命令信号消息结构
pub struct ComSemaphore {
    /// 聚合 Id
    pub agg_id: Uuid,
    /// 命令 Id
    pub com_id: Uuid,
    /// 命令数据
    pub com_data: Vec<u8>,
    /// 追踪跨度
    pub span: tracing::Span,
    /// 命令信号许可
    pub permit: OwnedSemaphorePermit,
}

/// 命令处理结果枚举
#[repr(u8)]
#[derive(Debug, PartialEq, ::bincode::Encode, ::bincode::Decode)]
pub enum Response {
    /// 成功
    Success = 0,
    /// 重复提交
    Duplicate = 1,
    /// 命令数据验证错误
    CheckError = 2,
    /// 序列化错误
    EncodeError = 3,
    /// 反序列化错误
    DecodeError = 4,
    /// 消息处理错误
    MsgError = 5,
    /// 写入事件流错误
    WriteError = 6,
    /// 读取事件流错误
    ReadError = 7,
    /// 写入命令流错误
    SendError = 8,
    /// 等待响应超时
    Timeout = 9,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bincode::config::{Fixint, Limit, LittleEndian};

    const SIZE: usize = 4;

    const BINCODE_HEADER: Configuration<LittleEndian, Fixint, Limit<SIZE>> =
        bincode::config::standard()
            .with_fixed_int_encoding()
            .with_limit::<SIZE>();

    #[test]
    fn serialize_and_deserialize_success() {
        let mut buf = [0u8; SIZE];
        if let Err(e) = bincode::encode_into_slice(Response::Success, &mut buf, BINCODE_HEADER) {
            println!("序列化出错：{e}");
        }
        let (res, _): (Response, _) = bincode::decode_from_slice(&buf, BINCODE_HEADER).unwrap();
        assert_eq!(res, Response::Success);
    }

    #[test]
    fn serialize_and_deserialize_duplicate() {
        let mut buf = [0u8; SIZE];
        if let Err(e) = bincode::encode_into_slice(Response::Duplicate, &mut buf, BINCODE_HEADER) {
            println!("序列化出错：{e}");
        }
        let (res, _): (Response, _) = bincode::decode_from_slice(&buf, BINCODE_HEADER).unwrap();
        assert_eq!(res, Response::Duplicate);
    }

    #[test]
    fn serialize_and_deserialize_check_error() {
        let mut buf = [0u8; SIZE];
        if let Err(e) = bincode::encode_into_slice(Response::CheckError, &mut buf, BINCODE_HEADER) {
            println!("序列化出错：{e}");
        }
        let (res, _): (Response, _) = bincode::decode_from_slice(&buf, BINCODE_HEADER).unwrap();
        assert_eq!(res, Response::CheckError);
    }

    #[test]
    fn serialize_and_deserialize_encode_error() {
        let mut buf = [0u8; SIZE];
        if let Err(e) = bincode::encode_into_slice(Response::EncodeError, &mut buf, BINCODE_HEADER)
        {
            println!("序列化出错：{e}");
        }
        let (res, _): (Response, _) = bincode::decode_from_slice(&buf, BINCODE_HEADER).unwrap();
        assert_eq!(res, Response::EncodeError);
    }

    #[test]
    fn serialize_and_deserialize_decode_error() {
        let mut buf = [0u8; SIZE];
        if let Err(e) = bincode::encode_into_slice(Response::DecodeError, &mut buf, BINCODE_HEADER)
        {
            println!("序列化出错：{e}");
        }
        let (res, _): (Response, _) = bincode::decode_from_slice(&buf, BINCODE_HEADER).unwrap();
        assert_eq!(res, Response::DecodeError);
    }

    #[test]
    fn serialize_and_deserialize_msg_error() {
        let mut buf = [0u8; SIZE];
        if let Err(e) = bincode::encode_into_slice(Response::MsgError, &mut buf, BINCODE_HEADER) {
            println!("序列化出错：{e}");
        }
        let (res, _): (Response, _) = bincode::decode_from_slice(&buf, BINCODE_HEADER).unwrap();
        assert_eq!(res, Response::MsgError);
    }

    #[test]
    fn serialize_and_deserialize_write_error() {
        let mut buf = [0u8; SIZE];
        if let Err(e) = bincode::encode_into_slice(Response::WriteError, &mut buf, BINCODE_HEADER) {
            println!("序列化出错：{e}");
        }
        let (res, _): (Response, _) = bincode::decode_from_slice(&buf, BINCODE_HEADER).unwrap();
        assert_eq!(res, Response::WriteError);
    }

    #[test]
    fn serialize_and_deserialize_read_error() {
        let mut buf = [0u8; SIZE];
        if let Err(e) = bincode::encode_into_slice(Response::ReadError, &mut buf, BINCODE_HEADER) {
            println!("序列化出错：{e}");
        }
        let (res, _): (Response, _) = bincode::decode_from_slice(&buf, BINCODE_HEADER).unwrap();
        assert_eq!(res, Response::ReadError);
    }

    #[test]
    fn serialize_and_deserialize_send_error() {
        let mut buf = [0u8; SIZE];
        if let Err(e) = bincode::encode_into_slice(Response::SendError, &mut buf, BINCODE_HEADER) {
            println!("序列化出错：{e}");
        }
        let (res, _): (Response, _) = bincode::decode_from_slice(&buf, BINCODE_HEADER).unwrap();
        assert_eq!(res, Response::SendError);
    }
}
