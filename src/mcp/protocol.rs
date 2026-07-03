//! MCP 协议类型定义 — JSON-RPC over stdin/stdout
//!
//! # 学习重点
//! - serde 的 Serialize/Deserialize derive — 自动 JSON 序列化
//! - `#[serde(skip_serializing_if = "Option::is_none")]` — 可选字段不输出
//! - `#[serde(rename = "type")]` — Rust 用 type_ 避开关键字

use serde::{Deserialize, Serialize};

// ============================================================
// JSON-RPC 请求
// ============================================================

/// JSON-RPC 2.0 请求
///
/// MCP 协议基于 JSON-RPC 2.0：
/// - 请求包含 method、params（可选）、id
/// - 响应包含 result（成功）或 error（失败），对应同一个 id
/// - 通知（没有 id）不需要响应
#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    /// 协议版本（固定 "2.0"）
    #[allow(dead_code)]
    pub jsonrpc: String,
    /// 方法名（如 "initialize"、"tools/list"、"tools/call"）
    pub method: String,
    /// 请求 ID（可以是数字或字符串）
    pub id: Option<serde_json::Value>,
    /// 方法参数
    pub params: Option<serde_json::Value>,
}

// ============================================================
// JSON-RPC 响应
// ============================================================

/// JSON-RPC 2.0 成功响应
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    /// 成功结果
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// 错误信息（成功时为 None）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 错误对象
#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    /// 创建成功响应
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// 创建错误响应
    pub fn error(id: Option<serde_json::Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }

    /// 序列化为 JSON 字符串
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

// ============================================================
// MCP 工具结果格式
// ============================================================

/// MCP 工具的返回结果
///
/// 格式要求：
/// ```json
/// {
///   "content": [
///     { "type": "text", "text": "结果内容" }
///   ],
///   "is_error": false
/// }
/// ```
#[derive(Debug, Serialize)]
pub struct ToolResult {
    pub content: Vec<ContentItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// MCP 内容项
#[derive(Debug, Serialize)]
pub struct ContentItem {
    #[serde(rename = "type")]
    pub type_: String,
    pub text: String,
}

impl ToolResult {
    /// 创建纯文本成功结果
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentItem {
                type_: "text".to_string(),
                text: text.into(),
            }],
            is_error: None,
        }
    }

    /// 创建错误结果
    pub fn error(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentItem {
                type_: "text".to_string(),
                text: text.into(),
            }],
            is_error: Some(true),
        }
    }
}
