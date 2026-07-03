//! 文件哈希表操作 — 用于增量导入
//!
//! # 什么是增量导入？
//! 第一次导入小说 → 扫描所有 .md 文件，提取角色/场景
//! 第二次再导入 → 只处理文件内容有变化的 .md，没变的跳过
//!
//! file_hashes 表记录了"这个文件上次导入时的 SHA-256 值"
//! 下次导入时算一遍新 hash，比较一下就知道文件有没有改过
//!
//! # 学习重点
//! - sha2 crate 计算 SHA-256
//! - 文件元数据（mtime, size）读取

use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::UNIX_EPOCH;

/// 文件的哈希和元数据
///
/// 对应 C 版的 cbm_file_hash_t
pub struct FileHash {
    pub project: String,
    pub rel_path: String,
    pub sha256: String,
    pub mtime_ns: i64,
    pub size: i64,
}

impl FileHash {
    /// 计算文件的 SHA-256 并返回 FileHash
    ///
    /// 💡 ? 运算符在这里处理 I/O 错误（文件不存在、读不了等）
    pub fn compute(project: &str, rel_path: &str, full_path: &Path) -> Result<Self, std::io::Error> {
        let metadata = std::fs::metadata(full_path)?;
        let mtime_ns = metadata.modified()?
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0);
        let size = metadata.len() as i64;

        let mut file = std::fs::File::open(full_path)?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher)?;
        let hash = format!("{:x}", hasher.finalize());

        Ok(Self {
            project: project.to_string(),
            rel_path: rel_path.to_string(),
            sha256: hash,
            mtime_ns,
            size,
        })
    }
}

// ============================================================
// FileHash 的 CRUD 方法
// ============================================================

impl super::Store {
    /// 获取文件哈希记录（如果没有导入过返回 None）
    pub fn get_file_hash(&self, project: &str, rel_path: &str) -> Result<Option<FileHash>, super::StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT project, rel_path, sha256, mtime_ns, size FROM file_hashes WHERE project = ?1 AND rel_path = ?2",
        )?;

        let mut rows = stmt.query_map(rusqlite::params![project, rel_path], |row| {
            Ok(FileHash {
                project: row.get("project")?,
                rel_path: row.get("rel_path")?,
                sha256: row.get("sha256")?,
                mtime_ns: row.get("mtime_ns")?,
                size: row.get("size")?,
            })
        })?;

        match rows.next() {
            Some(Ok(hash)) => Ok(Some(hash)),
            Some(Err(e)) => Err(super::StoreError::Sqlite(e)),
            None => Ok(None),
        }
    }

    /// 插入或更新文件哈希
    pub fn upsert_file_hash(&self, hash: &FileHash) -> Result<(), super::StoreError> {
        self.conn.execute(
            "INSERT INTO file_hashes (project, rel_path, sha256, mtime_ns, size)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(project, rel_path) DO UPDATE SET
                 sha256 = excluded.sha256,
                 mtime_ns = excluded.mtime_ns,
                 size = excluded.size",
            rusqlite::params![hash.project, hash.rel_path, hash.sha256, hash.mtime_ns, hash.size],
        )?;
        Ok(())
    }

    /// 删除某个项目的所有文件哈希（重新全量导入时用）
    pub fn delete_file_hashes_by_project(&self, project: &str) -> Result<(), super::StoreError> {
        self.conn.execute(
            "DELETE FROM file_hashes WHERE project = ?1",
            rusqlite::params![project],
        )?;
        Ok(())
    }
}
