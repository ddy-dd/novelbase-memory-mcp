use rusqlite::Connection;
use std::path::Path;
use thiserror::Error;
mod node;
mod edge;
pub mod file_hashes;
pub mod vectors;

//Store错误枚举
#[derive(Error, Debug)]
pub enum StoreError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Not Found")]
    NotFound,
    #[error("Other error: {0}")]
    Other(String),
}

//连接数据库
pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    #[cfg(test)]
    pub fn open_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        let store = Store { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// 初始化数据库表结构
    ///
    /// 💡 include_str!("schema.sql") — 编译时嵌入
    /// 把 schema.sql 的内容在编译时读取进来，变成 &str
    /// 运行时不需要读取文件——SQL 已经嵌在二进制里了
    fn init_schema(&self) -> Result<(), StoreError> {
        self.conn.execute_batch(include_str!("schema.sql"))?;

        // SQLite 默认不检查外键约束，需要显式开启（每个连接都要设）
        // 不加这行的话，外键约束虽然存在但不执行
        self.conn.execute_batch("PRAGMA foreign_keys = ON;")?;

        // FTS5 全文搜索表（仅在表不存在时创建）
        // 先用 CREATE IF NOT EXISTS 尝试，再验证列是否匹配
        // 如果旧表缺列（如 label/file_path），自动重建
        self.conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
                 name, qualified_name, label, file_path, project,
                 tokenize='unicode61 remove_diacritics 2'
             );"
        )?;
        // 检查旧表字段是否齐全（尝试插入 label 列，成功则匹配）
        let schema_ok = self.conn.execute_batch(
            "INSERT INTO nodes_fts(rowid, name, qualified_name, label, file_path, project)
             VALUES (-1, '','','','',''); DELETE FROM nodes_fts WHERE rowid = -1;"
        );
        if schema_ok.is_err() {
            // 旧表缺列 → 重建
            self.conn.execute_batch(
                "DROP TABLE IF EXISTS nodes_fts;
                 CREATE VIRTUAL TABLE IF NOT EXISTS nodes_fts USING fts5(
                     name, qualified_name, label, file_path, project,
                     tokenize='unicode61 remove_diacritics 2'
                 );"
            )?;
        }
        // 从已有节点重建搜索索引（幂等操作）
        let _ = self.conn.execute_batch(
            "INSERT OR IGNORE INTO nodes_fts(rowid, name, qualified_name, label, file_path, project)
             SELECT id, name, qualified_name, label, file_path, project FROM nodes;"
        );

        // 向量搜索：注册余弦函数 + 创建向量表
        let _ = vectors::register_cosine_function(&self.conn);
        let _ = vectors::create_vector_tables(&self.conn);

        Ok(())
    }

    /// 确保项目存在（如果不存在则创建）
    ///
    /// 💡 INSERT OR IGNORE — 如果 name 已存在就跳过
    ///    这样多次调用也不会报错
    pub fn ensure_project(&self, name: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT OR IGNORE INTO projects (name, root_path) VALUES (?1, ?2)",
            rusqlite::params![name, "."],
        )?;
        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_open_memory() {
        // 创建内存数据库——每次测试都是干净的
        let store = Store::open_memory().expect("应该能创建内存数据库");
        // 能创建出来就算成功，说明 schema 执行没问题
        assert!(store.conn.is_autocommit());
    }
}
