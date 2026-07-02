//! 数据模型 — 知识图谱中的节点和边
//!
//! # 学习重点
//! - `enum` 和 `match` 模式匹配
//! - `struct` 定义和方法
//! - `#[derive(...)]` 自动派生 trait
//! - `Option<T>` 表示可选值
//! - `pub` 控制可见性
//!
//! # 设计思路
//! 和 C 版的 codebase-memory-mcp 一样，我们用"节点 + 边"的图模型。
//! 但 Rust 的 enum 比 C 的 enum 强大得多——每个变体可以带不同的数据。
//!
//! 不过为了初学者友好，这里先用**标签字符串**区分节点类型（类似 C 版的
//! `properties_json` 方案），后续可以改成更类型安全的 enum 变体。

mod edge;
mod node;

pub use edge::{Edge, EdgeType};
pub use node::{Node, NodeLabel,Properties};
