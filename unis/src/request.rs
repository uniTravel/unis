#![allow(dead_code)]

use http::HeaderMap;
use uuid::Uuid;

/// 命令追踪键
#[derive(Clone, Debug)]
pub struct UniKey {
    /// 聚合 Id
    pub agg_id: Uuid,
    /// 聚合命令 Id
    pub com_id: [u8; 16],
    /// 追踪跨度 Id
    pub span_id: [u8; 8],
}

fn hex_char_to_byte(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn parse_hex<const N: usize>(hex_str: &str) -> Option<[u8; N]> {
    let bytes: Vec<u8> = hex_str
        .as_bytes()
        .chunks(2)
        .map(|chunk| {
            let high = hex_char_to_byte(chunk[0])?;
            let low = hex_char_to_byte(chunk[1])?;
            Some(high << 4 | low)
        })
        .collect::<Option<Vec<_>>>()?;

    bytes.try_into().ok()
}

fn parse_traceparent(tp: &str) -> Option<([u8; 16], [u8; 8])> {
    let parts: Vec<&str> = tp.split('-').collect();

    if parts.len() != 4 || parts[0] != "00" || parts[3] != "01" {
        return None;
    }

    let trace_id = parse_hex::<16>(parts[1])?;
    let span_id = parse_hex::<8>(parts[2])?;

    Some((trace_id, span_id))
}

pub(crate) fn extract_key(headers: &HeaderMap) -> Option<UniKey> {
    headers
        .get("traceparent")
        .and_then(|v| v.to_str().ok())
        .and_then(|tp| parse_traceparent(tp))
        .and_then(|(com_id, span_id)| {
            let agg_id = headers
                .get("x-agg-id")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| Uuid::parse_str(s).ok())
                .unwrap_or(Uuid::new_v4());
            Some(UniKey {
                agg_id,
                com_id,
                span_id,
            })
        })
}

/// Json 格式
pub struct JsonFormat;
/// Rkyv 格式
pub struct RkyvFormat;
