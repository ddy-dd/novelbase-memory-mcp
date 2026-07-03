//! MCP 服务器 — 通过 stdin/stdout 收发 JSON-RPC 消息
//!
//! # 学习重点
//! - BufReader / BufWriter — 缓冲 I/O
//! - serde_json 的 from_str / to_string
//! - match 分发 MCP 方法
//! - loop 事件循环

mod protocol;
mod tools;

use protocol::{JsonRpcRequest, JsonRpcResponse};
use serde_json::json;
use std::io::{BufRead, BufReader, Write};
use crate::store::Store;

// ============================================================
// MCP 服务器
// ============================================================

/// MCP 服务器 — 管理工具调用和 JSON-RPC 通信
///
/// 💡 MCP 的工作模式：
/// 1. 从 stdin 读一行 JSON →  读取请求
/// 2. 解析成 JSON-RPC 请求 →  解析请求
/// 3. match method 分发处理 → 处理业务
/// 4. 序列化成 JSON 写回 stdout → 返回响应
/// 5. 回到步骤 1
pub struct Server {
    store: Store,
}

impl Server {
    /// 创建 MCP 服务器实例
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    /// 启动事件循环（从 stdin 读取，写回 stdout）
    ///
    /// 💡 loop { ... } — 无限循环
    ///    直到 stdin 关闭（read_line 返回 0）才退出
    pub fn run(&self) -> anyhow::Result<()> {
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();

        // 💡 BufReader — 带缓冲的读取
        //    read_line 读到换行符才返回，效率比逐字节读高
        let mut reader = BufReader::new(stdin.lock());
        // 💡 stdout 需要 flush，因为 MCP 协议要求每条响应单独一行
        //    不 flush 的话数据可能还在缓冲区里
        let mut writer = stdout.lock();

        let mut line = String::new();

        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line)?;

            // stdin 关闭（EOF）→ 退出循环
            if bytes_read == 0 {
                break;
            }

            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            if let Some(response) = self.handle_request(line) {
                writeln!(writer, "{}", response)?;
                writer.flush()?;
            }
            // None = 通知（如 notifications/initialized），不需要回复
        }

        Ok(())
    }

    /// 处理一行 JSON-RPC 请求
    ///
    /// 💡 这是整个 MCP 服务器的核心——一个 match 分派所有方法
    ///    返回 None 表示这是个通知（不需要写响应）
    fn handle_request(&self, line: &str) -> Option<String> {
        // 1. 解析 JSON-RPC 请求
        let req: JsonRpcRequest = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => {
                return Some(JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e)).to_json());
            }
        };

        let id = req.id;

        // 2. 通知（没有 id）—— JSON-RPC 通知不需要响应
        //    比如 notifications/initialized 就是客户端发来的通知
        if id.is_none() {
            log::info!("收到 MCP 通知: {}", req.method);
            return None;
        }

        // 3. 按方法名分发
        let result = match req.method.as_str() {
            // MCP 协议初始化
            "initialize" => {
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "novelbase-memory-mcp",
                        "version": "0.1.0"
                    }
                })
            }
            // 健康检查
            "ping" => json!({}),
            // 列出所有工具
            "tools/list" => {
                let tools: Vec<serde_json::Value> = tools::all_tools().iter().map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": t.input_schema
                    })
                }).collect();
                json!({ "tools": tools })
            }
            // 调用某个工具
            "tools/call" => {
                let params = req.params.unwrap_or(json!({}));
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let tool_args = params.get("arguments").cloned().unwrap_or(json!({}));

                let result = tools::dispatch_tool(&self.store, tool_name, tool_args);
                json!(result)
            }
            // 未知方法
            _ => {
                return Some(JsonRpcResponse::error(id, -32601, format!("Method not found: {}", req.method)).to_json());
            }
        };

        // 4. 包装成 JSON-RPC 响应
        Some(JsonRpcResponse::success(id, result).to_json())
    }
}


