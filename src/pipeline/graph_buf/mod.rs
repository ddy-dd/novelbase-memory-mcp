//! GraphBuffer — 内存中的图缓冲区
//!
//! Pipeline 执行时，pass 先把结果写到内存里（GraphBuffer），
//! 最后统一刷到 SQLite（Store）。比每个 pass 都写一次数据库快。
//!
//! # 学习重点
//! - `HashMap` — Rust 的标准哈希表
//! - Entry API — `or_insert_with` 优雅处理插入/更新
//! - 迭代器 + 闭包 — `.values()`, `.filter()`, `.collect()`

use std::collections::HashMap;
use crate::models::{Edge, Node, NodeLabel};

/// 内存图缓冲区
///
/// 和 C 版的 cbm_gbuf_t 对应，但 Rust 用 HashMap 替代手写哈希表
pub struct GraphBuffer {
    /// 项目名
    project: String,
    /// 所有节点（id → Node）
    nodes: HashMap<i64, Node>,
    /// 所有边（id → Edge）
    edges: HashMap<i64, Edge>,
    /// qualified_name → id 的索引（用于快速查找）
    qn_index: HashMap<String, i64>,
    /// (source_id, target_id, type) → edge_id 的组合键索引
    ///
    /// 💡 对应 C 版的 edge_by_key 哈希表
    ///    用 `(src, tgt, type_str)` 三元组做 key
    ///    插入前先查这个表，如果 key 已存在就返回旧 ID，不重复插入
    edge_by_key: HashMap<(i64, i64, String), i64>,
    /// 下一个可用 ID
    next_id: i64,
}

impl GraphBuffer {
    /// 创建新的空图缓冲区
    pub fn new(project: &str) -> Self {
        Self {
            project: project.to_string(),
            nodes: HashMap::new(),
            edges: HashMap::new(),
            qn_index: HashMap::new(),
            edge_by_key: HashMap::new(),
            next_id: 1,
        }
    }

    /// 插入或更新节点
    ///
    /// 💡 Entry API — HashMap 最优雅的写法
    ///    qn_index.entry(qn) 返回一个 Entry（存在 or 不存在）
    ///    .or_insert_with(|| ...) 只有不存在时才执行闭包
    ///
    /// 相当于：
    ///   if qn 已存在 → 覆盖该 id 的节点
    ///   if qn 不存在 → 分配新 id，插入
    pub fn upsert_node(&mut self, mut node: Node) -> i64 {
        let qn = node.qualified_name.clone();
        let id = *self.qn_index.entry(qn.clone()).or_insert_with(|| {
            let id = self.next_id;
            self.next_id += 1;
            id
        });

        node.id = id;
        self.nodes.insert(id, node);
        id
    }

    /// 按 qualified_name 查找节点
    pub fn find_by_qn(&self, qn: &str) -> Option<&Node> {
        self.qn_index.get(qn).and_then(|id| self.nodes.get(id))
    }

    /// 按标签查找节点（返回引用列表）
    pub fn find_by_label(&self, label: NodeLabel) -> Vec<&Node> {
        self.nodes.values()
            .filter(|n| n.label == label)
            .collect()
    }

    /// 插入边（有去重）
    ///
    /// 💡 先查 edge_by_key，如果 (source_id, target_id, type) 已存在
    ///    就直接返回旧 ID，不创建新边——防止 repeat 插入
    ///    对应 C 版的 cbm_gbuf_insert_edge 去重逻辑
    pub fn insert_edge(&mut self, mut edge: Edge) -> i64 {
        let key = (edge.source_id, edge.target_id, edge.edge_type.as_str().to_string());
        if let Some(&existing_id) = self.edge_by_key.get(&key) {
            return existing_id;
        }
        let id = self.next_id;
        self.next_id += 1;
        edge.id = id;
        self.edge_by_key.insert(key, id);
        self.edges.insert(id, edge);
        id
    }

    /// 获取节点数量
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 获取边数量
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// 把所有数据刷到 SQLite
    ///
    /// 💡 对应 C 版的 cbm_gbuf_dump_to_sqlite
    ///    先 upsert 所有 node，把 GraphBuffer 里的临时 ID 换成 SQLite 的真实 ID
    ///    再 insert edge（此时 source_id / target_id 才指向正确的节点）
    pub fn dump_to_store(&mut self, store: &crate::store::Store) -> Result<(), crate::store::StoreError> {
        // 1. 先写所有节点，记录 GraphBuffer ID → SQLite ID 的映射
        let mut id_map: std::collections::HashMap<i64, i64> = std::collections::HashMap::new();
        for node in self.nodes.values_mut() {
            let new_id = store.upsert_node(node)?;
            id_map.insert(node.id, new_id);
            node.id = new_id;
        }

        // 2. 更新边的 ID 引用，再写 edge
        for edge in self.edges.values_mut() {
            if let Some(&new_src) = id_map.get(&edge.source_id) {
                edge.source_id = new_src;
            }
            if let Some(&new_tgt) = id_map.get(&edge.target_id) {
                edge.target_id = new_tgt;
            }
            store.insert_edge(edge)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Edge, EdgeType, NodeLabel, Node};

    #[test]
    fn test_graph_buffer_new() {
        let gb = GraphBuffer::new("test");
        assert_eq!(gb.node_count(), 0);
        assert_eq!(gb.edge_count(), 0);
    }

    #[test]
    fn test_upsert_node_find() {
        let mut gb = GraphBuffer::new("test");
        let node = Node::new("test", NodeLabel::Character, "张三", "test.张三");
        let id = gb.upsert_node(node);

        assert_eq!(id, 1);
        let found = gb.find_by_qn("test.张三");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "张三");
    }

    #[test]
    fn test_upsert_node_dedup() {
        let mut gb = GraphBuffer::new("test");

        let id1 = gb.upsert_node(Node::new("test", NodeLabel::Character, "张三", "test.张三"));
        let id2 = gb.upsert_node(Node::new("test", NodeLabel::Character, "张三（改）", "test.张三"));

        // 同一个 qualified_name → 返回同一个 id
        assert_eq!(id1, id2);
        // 节点被更新
        assert_eq!(gb.find_by_qn("test.张三").unwrap().name, "张三（改）");
    }

    #[test]
    fn test_find_by_label() {
        let mut gb = GraphBuffer::new("test");
        gb.upsert_node(Node::new("test", NodeLabel::Character, "张三", "test.张三"));
        gb.upsert_node(Node::new("test", NodeLabel::Location, "北京", "test.北京"));

        let chars = gb.find_by_label(NodeLabel::Character);
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].name, "张三");
    }

    #[test]
    fn test_insert_edge_dedup() {
        let mut gb = GraphBuffer::new("test");

        // 先插两个节点，再尝试插入相同的边两次
        let n1 = gb.upsert_node(Node::new("test", NodeLabel::Character, "A", "test.A"));
        let n2 = gb.upsert_node(Node::new("test", NodeLabel::Character, "B", "test.B"));

        let edge = Edge::new("test", n1, n2, EdgeType::Knows);
        let id1 = gb.insert_edge(edge);
        let id2 = gb.insert_edge(Edge::new("test", n1, n2, EdgeType::Knows));

        // 两条边应该返回同一个 ID（去重了）
        assert_eq!(id1, id2);
        assert_eq!(gb.edge_count(), 1);
    }
}
