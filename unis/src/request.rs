use http::HeaderMap;
use uuid::Uuid;

/// 命令请求的路径参数
#[derive(Clone, Debug)]
pub struct UniKey {
    /// 聚合 Id
    pub agg_id: Uuid,
    /// 聚合命令 Id
    pub com_id: Uuid,
}

fn parse_traceparent(tp: &str) -> Result<Uuid, String> {
    if tp.is_empty() {
        return Err("traceparent 为空".into());
    }

    let parts: Vec<&str> = tp.split('-').collect();
    if parts.len() != 4 {
        return Err(format!(
            "格式错误，需要 4 段，实际 {} 段: '{}'",
            parts.len(),
            tp
        ));
    }

    if parts[0] != "00" {
        return Err(format!("不支持的版本号: '{}'", parts[0]));
    }

    if parts[1].len() != 32 {
        return Err(format!(
            "trace_id 长度错误，需要 32 hex，实际 {} 字符: '{}'",
            parts[1].len(),
            parts[1]
        ));
    }

    if !parts[1].chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("trace_id 包含非法字符: '{}'", parts[1]));
    }

    if parts[1] == "00000000000000000000000000000000" {
        return Err("trace_id 为全零，无效".into());
    }

    let uuid_str = format!(
        "{}-{}-{}-{}-{}",
        &parts[1][0..8],
        &parts[1][8..12],
        &parts[1][12..16],
        &parts[1][16..20],
        &parts[1][20..32]
    );
    Uuid::parse_str(&uuid_str).map_err(|e| format!("无法解析为 UUID: {}", e))
}

pub(crate) fn extract_key(headers: &HeaderMap) -> Option<UniKey> {
    if let Some(tp) = headers.get("traceparent").and_then(|v| v.to_str().ok()) {
        match parse_traceparent(tp) {
            Ok(com_id) => {
                let agg_id = headers
                    .get("x-agg-id")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .unwrap_or(Uuid::new_v4());
                return Some(UniKey { agg_id, com_id });
            }
            Err(e) => {
                tracing::warn!("traceparent 格式错误: value='{}', error={}", tp, e);
                return None;
            }
        }
    }

    None
}

/// Json 格式
pub struct JsonFormat;
/// Rkyv 格式
pub struct RkyvFormat;
