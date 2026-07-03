//! MCP 工具定义和分发
//!
//! # 学习重点
//! - 函数指针风格的 trait 设计
//! - serde_json::Value 的动态操作
//! - match 代替 if-else 链实现分发

use super::protocol::ToolResult;
use crate::models::{Edge, EdgeType, Node, NodeLabel};
use crate::store::Store;
use serde_json::json;

// ============================================================
// 工具定义
// ============================================================

/// MCP 工具描述
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: serde_json::Value,
}

/// 返回所有可用工具
pub fn all_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "add_character",
            description: "添加角色到知识图谱",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project": {"type": "string", "description": "项目名"},
                    "name": {"type": "string", "description": "角色名"},
                    "traits": {"type": "string", "description": "角色特征"}
                },
                "required": ["project", "name"]
            }),
        },
        ToolDef {
            name: "list_characters",
            description: "列出项目中的所有角色",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project": {"type": "string", "description": "项目名"}
                },
                "required": ["project"]
            }),
        },
        ToolDef {
            name: "add_relationship",
            description: "添加两个角色之间的关系",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project": {"type": "string"},
                    "character_a": {"type": "string", "description": "角色A的名字"},
                    "character_b": {"type": "string", "description": "角色B的名字"},
                    "relationship_type": {
                        "type": "string",
                        "enum": ["knows", "located_in", "appears_in", "leads_to", "part_of", "happens_at", "mentions", "related_to"],
                        "description": "关系类型"
                    }
                },
                "required": ["project", "character_a", "character_b", "relationship_type"]
            }),
        },
        ToolDef {
            name: "search_graph",
            description: "按标签搜索知识图谱中的节点",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project": {"type": "string"},
                    "label": {
                        "type": "string",
                        "enum": ["character", "location", "scene", "chapter", "plotline", "timeline", "item", "note"],
                        "description": "节点标签"
                    }
                },
                "required": ["project", "label"]
            }),
        },
    ]
}

// ============================================================
// 工具参数解析（serde 自动反序列化）
// ============================================================

#[derive(serde::Deserialize)]
struct AddCharacterArgs {
    project: String,
    name: String,
    traits: Option<String>,
}

#[derive(serde::Deserialize)]
struct ListCharactersArgs {
    project: String,
}

#[derive(serde::Deserialize)]
struct AddRelationshipArgs {
    project: String,
    character_a: String,
    character_b: String,
    relationship_type: String,
}

#[derive(serde::Deserialize)]
struct SearchGraphArgs {
    project: String,
    label: String,
}

// ============================================================
// 工具分发与处理
// ============================================================

/// 调用某个工具，返回 JSON 格式的结果
///
/// 💡 参数 args 已经是解析好的 JSON Value
///    用 serde_json::from_value 可以进一步反序列化成具体类型
pub fn dispatch_tool(store: &Store, tool_name: &str, args: serde_json::Value) -> ToolResult {
    match tool_name {
        "add_character" => handle_add_character(store, args),
        "list_characters" => handle_list_characters(store, args),
        "add_relationship" => handle_add_relationship(store, args),
        "search_graph" => handle_search_graph(store, args),
        _ => ToolResult::error(format!("未知工具: {}", tool_name)),
    }
}

/// 用 StoreError → ToolResult 的转换辅助
fn store_result<T>(
    result: Result<T, crate::store::StoreError>,
    ok_msg: impl FnOnce(&T) -> String,
) -> ToolResult {
    match result {
        Ok(val) => ToolResult::text(ok_msg(&val)),
        Err(e) => ToolResult::error(format!("存储错误: {}", e)),
    }
}

/// 确保项目存在（自动创建），失败则返回错误
fn ensure_project(store: &Store, name: &str) -> Result<(), ToolResult> {
    store.ensure_project(name).map_err(|e| ToolResult::error(format!("创建项目失败: {}", e)))
}

/// 添加角色
fn handle_add_character(store: &Store, args: serde_json::Value) -> ToolResult {
    let params: AddCharacterArgs = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => return ToolResult::error(format!("参数解析失败: {}", e)),
    };
    if let Err(e) = ensure_project(store, &params.project) {
        return e;
    }

    let qn = format!("{}.{}", params.project, params.name);
    let mut node = Node::new(&params.project, NodeLabel::Character, &params.name, &qn);
    if let Some(t) = params.traits {
        node.properties.insert("traits", t);
    }

    store_result(store.upsert_node(&node), |id| {
        format!("角色 '{}' 已添加 (ID: {})", params.name, id)
    })
}

/// 列出角色
fn handle_list_characters(store: &Store, args: serde_json::Value) -> ToolResult {
    let params: ListCharactersArgs = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => return ToolResult::error(format!("参数解析失败: {}", e)),
    };
    if let Err(e) = ensure_project(store, &params.project) {
        return e;
    }

    match store.find_nodes_by_label(&params.project, NodeLabel::Character) {
        Ok(characters) => {
            if characters.is_empty() {
                ToolResult::text(format!("项目 '{}' 中没有角色", params.project))
            } else {
                let mut lines = format!("📋 角色列表（项目: {}）:\n", params.project);
                for c in &characters {
                    lines.push_str(&format!("  - {} (ID: {})\n", c.name, c.id));
                }
                lines.push_str(&format!("共 {} 个角色", characters.len()));
                ToolResult::text(lines)
            }
        }
        Err(e) => ToolResult::error(format!("查询失败: {}", e)),
    }
}

/// 添加关系
fn handle_add_relationship(store: &Store, args: serde_json::Value) -> ToolResult {
    let params: AddRelationshipArgs = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => return ToolResult::error(format!("参数解析失败: {}", e)),
    };
    if let Err(e) = ensure_project(store, &params.project) {
        return e;
    }

    let a_qn = format!("{}.{}", params.project, params.character_a);
    let b_qn = format!("{}.{}", params.project, params.character_b);

    let a = match store.find_node_by_qn(&params.project, &a_qn) {
        Ok(Some(n)) => n,
        Ok(None) => return ToolResult::error(format!("未找到角色: {}", params.character_a)),
        Err(e) => return ToolResult::error(format!("查询失败: {}", e)),
    };

    let b = match store.find_node_by_qn(&params.project, &b_qn) {
        Ok(Some(n)) => n,
        Ok(None) => return ToolResult::error(format!("未找到角色: {}", params.character_b)),
        Err(e) => return ToolResult::error(format!("查询失败: {}", e)),
    };

    let edge_type = match EdgeType::from_str(&params.relationship_type) {
        Some(t) => t,
        None => return ToolResult::error(format!("未知关系类型: {}", params.relationship_type)),
    };

    let edge = Edge::new(&params.project, a.id, b.id, edge_type);
    store_result(store.insert_edge(&edge), |id| {
        format!("关系已添加 (ID: {}) — {} --[{}]--> {}", id, params.character_a, params.relationship_type.to_uppercase(), params.character_b)
    })
}

/// 搜索图谱
fn handle_search_graph(store: &Store, args: serde_json::Value) -> ToolResult {
    let params: SearchGraphArgs = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => return ToolResult::error(format!("参数解析失败: {}", e)),
    };

    let node_label = match NodeLabel::from_str(&params.label) {
        Some(l) => l,
        None => return ToolResult::error(format!("未知标签: {}", params.label)),
    };

    match store.find_nodes_by_label(&params.project, node_label) {
        Ok(nodes) => {
            if nodes.is_empty() {
                ToolResult::text(format!("没有找到 [{}] 类型的节点", params.label))
            } else {
                let mut lines = format!("📋 找到 {} 个 [{}] 节点:\n", nodes.len(), params.label);
                for n in &nodes {
                    lines.push_str(&format!("  - {} (ID: {})\n", n.name, n.id));
                }
                ToolResult::text(lines)
            }
        }
        Err(e) => ToolResult::error(format!("查询失败: {}", e)),
    }
}
