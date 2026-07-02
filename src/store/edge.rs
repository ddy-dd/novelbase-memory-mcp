//! Edge 的数据库操作（CRUD）
//!
//! # 学习重点
//! - 和 node.rs 同样的模式再练一遍
//! - 新概念：`find_edges_for_node` 用 SQL OR 条件

use crate::models::{Edge, EdgeType, Properties};

// ============================================================
// 工具函数：把 SQLite 的一行转成 Edge
// ============================================================

fn row_2_edge(row: &rusqlite::Row) -> rusqlite::Result<Edge> {
    let properties_str: String = row.get("properties")?;
    let properties = Properties::from_json(&properties_str)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    let type_str: String = row.get("type")?;
    let edge_type = EdgeType::from_str(&type_str).ok_or_else(|| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("未知边类型: {}", type_str))
        ))
    })?;

    Ok(Edge {
        id: row.get("id")?,
        project: row.get("project")?,
        source_id: row.get("source_id")?,
        target_id: row.get("target_id")?,
        edge_type,
        properties,
    })
}

// ============================================================
// Store 的 Edge CRUD 方法
// ============================================================

impl super::Store {
    /// 插入新边
    ///
    /// 💡 这里用 INSERT 而非 UPSERT
    ///    因为边不像节点有 qualified_name 作为唯一键
    ///    schema.sql 里 UNIQUE(source_id, target_id, type) 的组合唯一约束
    ///    如果冲突直接报错，不自动覆盖
    pub fn insert_edge(&self, edge: &Edge) -> Result<i64, super::StoreError> {
        self.conn.execute(
            "INSERT INTO edges (project, source_id, target_id, type, properties)
               VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                  edge.project,
                  edge.source_id,
                  edge.target_id,
                  edge.edge_type.as_str(),
                  edge.properties.to_json(),
              ],
        )?;

        // 返回自增的 id
        Ok(self.conn.last_insert_rowid())
    }

    /// 按 ID 查找边
    pub fn find_edge_by_id(&self, id: i64) -> Result<Option<Edge>, super::StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project, source_id, target_id, type, properties
               FROM edges WHERE id = ?1",
        )?;

        let mut rows = stmt.query_map(rusqlite::params![id], row_2_edge)?;
        let edge = rows.next().transpose()?;
        Ok(edge)
    }

    /// 查找与某节点相连的所有边（包括 source 和 target）
    ///
    /// 💡 SQL 的 OR 条件：id 是源节点 OR 目标节点
    ///    这样一次查询就能拿到所有关联关系
    pub fn find_edges_for_node(&self, node_id: i64) -> Result<Vec<Edge>, super::StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project, source_id, target_id, type, properties
               FROM edges WHERE source_id = ?1 OR target_id = ?1
               ORDER BY id",
        )?;

        let rows = stmt.query_map(rusqlite::params![node_id], row_2_edge)?;
        let edges: Vec<Edge> = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(edges)
    }

    /// 按类型查找边
    pub fn find_edges_by_type(&self, project: &str, edge_type: EdgeType) -> Result<Vec<Edge>, super::StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project, source_id, target_id, type, properties
               FROM edges WHERE project = ?1 AND type = ?2
               ORDER BY id",
        )?;

        let rows = stmt.query_map(rusqlite::params![project, edge_type.as_str()], row_2_edge)?;
        let edges: Vec<Edge> = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(edges)
    }

    /// 删除边
    pub fn delete_edge(&self, id: i64) -> Result<bool, super::StoreError> {
        let affected = self.conn.execute(
            "DELETE FROM edges WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(affected > 0)
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::super::Store;
    use crate::models::{Edge, EdgeType, Node, NodeLabel};

    // 测试辅助函数：创建一个项目 + 两个节点，返回 (store, id_a, id_b)
    fn setup_graph() -> (Store, i64, i64) {
        let mut store = Store::open_memory().expect("创建内存数据库");
        store.ensure_project("novel").unwrap();

        let a = store.upsert_node(&Node::new("novel", NodeLabel::Character, "张三", "novel.张三")).unwrap();
        let b = store.upsert_node(&Node::new("novel", NodeLabel::Character, "李四", "novel.李四")).unwrap();
        (store, a, b)
    }

    #[test]
    fn test_insert_and_find_edge() {
        let (store, a_id, b_id) = setup_graph();

        let edge = Edge::new("novel", a_id, b_id, EdgeType::Knows);
        let id = store.insert_edge(&edge).expect("插入边");

        let found = store.find_edge_by_id(id)
            .expect("查询应该成功")
            .expect("应该找到");

        assert_eq!(found.source_id, a_id);
        assert_eq!(found.target_id, b_id);
        assert_eq!(found.edge_type, EdgeType::Knows);
    }

    #[test]
    fn test_find_edges_for_node() {
        let (store, a_id, b_id) = setup_graph();

        // 插入两条边：李四 → 张三 和 张三 → 李四
        store.insert_edge(&Edge::new("novel", a_id, b_id, EdgeType::Knows)).unwrap();
        store.insert_edge(&Edge::new("novel", b_id, a_id, EdgeType::Knows)).unwrap();

        // 张三相关的边应返回 2 条
        let edges = store.find_edges_for_node(a_id).expect("查询边");
        assert_eq!(edges.len(), 2);
    }

    #[test]
    fn test_find_edges_by_type() {
        let (store, a_id, b_id) = setup_graph();

        store.insert_edge(&Edge::new("novel", a_id, b_id, EdgeType::Knows)).unwrap();
        store.insert_edge(&Edge::new("novel", a_id, b_id, EdgeType::RelatedTo)).unwrap();

        let knows = store.find_edges_by_type("novel", EdgeType::Knows).expect("查询 KNOWS");
        assert_eq!(knows.len(), 1);
        assert_eq!(knows[0].edge_type, EdgeType::Knows);
    }

    #[test]
    fn test_delete_edge() {
        let (store, a_id, b_id) = setup_graph();
        let id = store.insert_edge(&Edge::new("novel", a_id, b_id, EdgeType::Knows)).unwrap();

        assert!(store.delete_edge(id).expect("删除应该成功"));
        assert!(store.find_edge_by_id(id).expect("查询成功").is_none());
        assert!(!store.delete_edge(999).expect("删除不存在应该返回 false"));
    }
}
