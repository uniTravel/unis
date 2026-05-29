//! # **unis** 的 Kafka 实现
//!
//!
#![warn(missing_docs)]
#![allow(dead_code)]

mod config;

#[cfg(feature = "projector")]
pub mod projector;
#[cfg(feature = "sender")]
pub mod sender;
#[cfg(feature = "subscriber")]
pub mod subscriber;

use rdkafka::{
    Message,
    message::{BorrowedHeaders, BorrowedMessage, Headers},
};
use unis::{UniResponse, errors::UniError};
use uuid::Uuid;

#[inline(always)]
fn get_agg_key(msg: &BorrowedMessage<'_>) -> Result<Uuid, UniError> {
    let key = msg.key().ok_or("消息键不存在")?;
    Ok(Uuid::from_slice(key).map_err(|e| e.to_string())?)
}

#[inline(always)]
fn get_com_key(msg: &BorrowedMessage<'_>) -> Result<[u8; 16], UniError> {
    let key = msg.key().ok_or("消息键不存在")?;
    Ok(key
        .try_into()
        .map_err(|e| format!("提取'com_id'失败：{e}"))?)
}

#[inline(always)]
fn get_com_id(headers: &BorrowedHeaders) -> Result<[u8; 16], UniError> {
    Ok(headers
        .iter()
        .find(|h| h.key == "com_id")
        .ok_or("键为'com_id'的消息头不存在")?
        .value
        .ok_or("键'com_id'对应的值为空")?
        .try_into()
        .map_err(|e| format!("提取'com_id'失败：{e}"))?)
}

#[inline(always)]
fn get_span_id(headers: &BorrowedHeaders) -> Result<[u8; 8], UniError> {
    Ok(headers
        .iter()
        .find(|h| h.key == "span_id")
        .ok_or("键为'span_id'的消息头不存在")?
        .value
        .ok_or("键'span_id'对应的值为空")?
        .try_into()
        .map_err(|e| format!("提取'span_id'失败：{e}"))?)
}

fn get_trace_flags(headers: &BorrowedHeaders) -> Result<u8, UniError> {
    Ok(headers
        .iter()
        .find(|h| h.key == "trace_flags")
        .ok_or("键为'trace_flags'的消息头不存在")?
        .value
        .ok_or("键'trace_flags'对应的值为空")?[0])
}

#[inline(always)]
fn get_response(headers: &BorrowedHeaders) -> Result<UniResponse, UniError> {
    let res_data = headers
        .iter()
        .find(|h| h.key == "response")
        .ok_or("键为'response'的消息头不存在")?
        .value
        .ok_or("键'response'对应的值为空")?;
    Ok(UniResponse::from_bytes(res_data))
}
