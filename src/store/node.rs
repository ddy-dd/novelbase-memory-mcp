use crate::models::{Node, NodeLabel, Properties};

/// 从 SQLite 行数据构造 Node
///
/// 💡 query_map 要求行处理函数返回 `rusqlite::Result<Node>`
///    即错误类型必须是 `rusqlite::Error`，不能是自定义的 `StoreError`
///
/// 那非 SQLite 的错误怎么办？
/// - JSON 解析失败 → ToSqlConversionFailure 包一下
/// - 未知标签 → 同理包一下
fn row_2_node(row: &rusqlite::Row) -> rusqlite::Result<Node> {
    let properties_str: String = row.get("properties")?;
    let properties = Properties::from_json(&properties_str)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    // 💡 注意列名是 "label" 不是 "labels"——schema.sql 里定义的
    let label_str: String = row.get("label")?;
    let label = NodeLabel::from_str(&label_str).ok_or_else(|| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(
            std::io::Error::new(std::io::ErrorKind::InvalidData, format!("未知标签: {}", label_str))
        ))
    })?;

    Ok(Node {
        id: row.get("id")?,
        project: row.get("project")?,
        label,
        name: row.get("name")?,
        qualified_name: row.get("qualified_name")?,
        file_path: row.get("file_path")?,
        start_line: row.get("start_line")?,
        end_line: row.get("end_line")?,
        properties,
    })
}

impl super::Store{
    //插入或更新节点
    pub fn upsert_node(&mut self, node: &Node) -> Result<i64, super::StoreError> {
        self.conn.execute(
            "INSERT INTO nodes (project, label, name, qualified_name, file_path, start_line, end_line, properties)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
               ON CONFLICT(qualified_name) DO UPDATE SET
                   label = excluded.label,
                   name = excluded.name,
                   file_path = excluded.file_path,
                   start_line = excluded.start_line,
                   end_line = excluded.end_line,
                   properties = excluded.properties",
            rusqlite::params![
                  node.project,
                  node.label.as_str(),      // NodeLabel → &str
                  node.name,
                  node.qualified_name,
                  node.file_path,            // Option<String> 自动变成 NULL
                  node.start_line,           // Option<i32> 自动变成 NULL
                  node.end_line,
                  node.properties.to_json(), // Properties → JSON String
              ],
        )?;

        let id = self.conn.query_row(
            "SELECT id FROM nodes WHERE qualified_name = ?1",
            rusqlite::params![node.qualified_name],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(id)
    }

    //按qn查找节点
    pub fn find_node_by_qn(&self, project: &str, qn: &str) -> Result<Option<Node>, super::StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project, label, name, qualified_name, file_path, start_line, end_line, properties
               FROM nodes WHERE project = ?1 AND qualified_name = ?2",
        )?;

        let mut rows = stmt.query_map(rusqlite::params![project, qn], row_2_node)?;
        let node = rows.next().transpose()?;
        Ok(node)
    }

    //按id查找
    pub fn find_node_by_id(&self, id: i64) -> Result<Option<Node>, super::StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project, label, name, qualified_name, file_path, start_line, end_line, properties
               FROM nodes WHERE id = ?1",
        )?;

        let mut rows = stmt.query_map(rusqlite::params![id], row_2_node)?;
        let node = rows.next().transpose()?;
        Ok(node)
    }

    //按标签查找
    pub fn find_nodes_by_label(&self, project: &str, label: NodeLabel) -> Result<Vec<Node>, super::StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project, label, name, qualified_name, file_path, start_line, end_line, properties
               FROM nodes WHERE project = ?1 AND label = ?2
               ORDER BY name",
        )?;

        let rows = stmt.query_map(rusqlite::params![project, label.as_str()], row_2_node)?;

        // 💡 collect() 收集迭代器到 Vec
        //    但 rows 是 Result 的迭代器，collect 还需要处理错误
        //    用 collect::<Result<Vec<_>, _>>() 这种"魔术"写法
        // 💡 collect 到这里是 Result<Vec<Node>, rusqlite::Error>
        //    用 ? 自动转成 StoreError（thiserror 的 #[from] 处理了转换）
        let nodes: Vec<Node> = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(nodes)
    }

    /// 删除节点
    pub fn delete_node(&self, id: i64) -> Result<bool, super::StoreError> {
        let affected = self.conn.execute(
            "DELETE FROM nodes WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(affected > 0) // 如果影响了 0 行，说明没找到
    }
}

#[cfg(test)]
mod tests {
    use super::super::Store;
    use crate::models::{Node, NodeLabel};

    #[test]
    fn test_upsert_and_find_by_qn() {
        let mut store = Store::open_memory().expect("创建内存数据库");
        store.ensure_project("test_project").unwrap();

        let node = Node::new("test_project", NodeLabel::Character, "张三", "test_project.张三");
        let id = store.upsert_node(&node).expect("插入节点");

        let found = store.find_node_by_qn("test_project", "test_project.张三")
            .expect("查询应该成功")
            .expect("应该找到节点");

        assert_eq!(found.name, "张三");
        assert_eq!(found.label, NodeLabel::Character);
        assert_eq!(found.id, id);
    }

    #[test]
    fn test_find_nonexistent() {
        let store = Store::open_memory().expect("创建内存数据库");

        let result = store.find_node_by_qn("test_project", "不存在的")
            .expect("查询应该成功");

        assert!(result.is_none());
    }

    #[test]
    fn test_find_by_label() {
        let mut store = Store::open_memory().expect("创建内存数据库");
        store.ensure_project("p1").unwrap();

        // 插入两个角色和一个地点
        store.upsert_node(&Node::new("p1", NodeLabel::Character, "张三", "p1.张三")).unwrap();
        store.upsert_node(&Node::new("p1", NodeLabel::Character, "李四", "p1.李四")).unwrap();
        store.upsert_node(&Node::new("p1", NodeLabel::Location, "北京", "p1.北京")).unwrap();

        let chars = store.find_nodes_by_label("p1", NodeLabel::Character)
            .expect("查询角色");

        assert_eq!(chars.len(), 2);
        assert!(chars.iter().any(|n| n.name == "张三"));
        assert!(chars.iter().any(|n| n.name == "李四"));
    }

    #[test]
    fn test_upsert_updates_existing() {
        let mut store = Store::open_memory().expect("创建内存数据库");
        store.ensure_project("p1").unwrap();

        store.upsert_node(&Node::new("p1", NodeLabel::Character, "张三", "p1.张三")).unwrap();

        // 改名字再插入（same qualified_name = 更新）
        let mut node = Node::new("p1", NodeLabel::Character, "张三（改）", "p1.张三");
        node.properties.insert("alias", "小张");
        store.upsert_node(&node).unwrap();

        let found = store.find_node_by_qn("p1", "p1.张三")
            .expect("查询成功")
            .expect("应该存在");

        assert_eq!(found.name, "张三（改）");
        assert_eq!(found.properties.get_str("alias"), Some("小张"));
    }

    #[test]
    fn test_delete_node() {
        let mut store = Store::open_memory().expect("创建内存数据库");
        store.ensure_project("p1").unwrap();
        let id = store.upsert_node(&Node::new("p1", NodeLabel::Character, "张三", "p1.张三")).unwrap();

        assert!(store.delete_node(id).expect("删除应该成功"));
        assert!(store.find_node_by_id(id).expect("查询成功").is_none());
        // 删不存在的节点返回 false
        assert!(!store.delete_node(999).expect("删除不存在节点应该返回 false"));
    }
}
