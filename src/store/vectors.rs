//! 向量存储与语义搜索 — 对齐 codebase-memory-mcp 的完整实现
//!
//! 架构：
//!   node_vectors 表 — 节点向量（int8 BLOB，量化压缩）
//!   token_vectors 表 — 关键词向量 + IDF（用于查询时查表）
//!   cbm_cosine_i8() — SQLite 自定义函数（int8 余弦相似度）
//!   vs_min_cosine_score — 多关键词取最小分（C 版 vs_min_cosine_score）
//!   vs_fill_sparse_random — OOV 回退（XXH3 稀疏随机投影）

use rusqlite::{params, Connection};

/// 向量维度（由使用的模型决定）
pub const VEC_DIM: usize = 512;

/// int8 量化 — f32 向量转为 int8 BLOB
///
/// 对齐 C 版 vs_normalize_and_quantize：
/// 1. 找最大绝对值作为缩放因子
/// 2. 缩放到 [-127, 127] 范围
/// 3. 裁剪越界值
pub fn quantize_i8(vec: &[f32]) -> Vec<u8> {
    if vec.is_empty() {
        return Vec::new();
    }
    let max = vec.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
    if max < 1e-10 {
        return vec![0u8; vec.len()];
    }
    let scale = 127.0 / max;
    vec.iter()
        .map(|v| {
            let q = (v * scale).round() as i8;
            q.to_le_bytes()[0] // i8 → u8（保持位模式）
        })
        .collect()
}

/// 反量化 — int8 BLOB → f32 向量
pub fn dequantize_f32(blob: &[u8]) -> Vec<f32> {
    blob.iter()
        .map(|&b| i8::from_le_bytes([b]) as f32 / 127.0)
        .collect()
}

// ═══════════════════════════════════════════════════════════════════
// SQLite 自定义函数：int8 余弦相似度
// 对齐 C 版的 sqlite_cosine_i8
// ═══════════════════════════════════════════════════════════════════

/// 注册余弦相似度函数到 SQLite
pub fn register_cosine_function(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.create_scalar_function(
        "cbm_cosine_i8",
        2,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8
            | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let a = match ctx.get_raw(0).as_blob() {
                Ok(b) => b,
                Err(_) => return Ok(0.0_f64),
            };
            let b = match ctx.get_raw(1).as_blob() {
                Ok(b) => b,
                Err(_) => return Ok(0.0_f64),
            };
            if a.len() != b.len() || a.is_empty() {
                return Ok(0.0_f64);
            }
            // int8 点积
            let mut dot = 0_i32;
            let mut ma = 0_i32;
            let mut mb = 0_i32;
            for i in 0..a.len() {
                let va = a[i] as i8 as i32;
                let vb = b[i] as i8 as i32;
                dot += va * vb;
                ma += va * va;
                mb += vb * vb;
            }
            let denom = ((ma as f64).sqrt() * (mb as f64).sqrt()).max(1e-10);
            Ok(dot as f64 / denom)
        },
    )?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// 表结构
// ═══════════════════════════════════════════════════════════════════

/// 创建向量相关表
pub fn create_vector_tables(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS node_vectors (
            node_id INTEGER PRIMARY KEY,
            project TEXT NOT NULL,
            vector BLOB NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_nv_project ON node_vectors(project);

        CREATE TABLE IF NOT EXISTS token_vectors (
            id INTEGER PRIMARY KEY,
            project TEXT NOT NULL,
            token TEXT NOT NULL,
            vector BLOB NOT NULL,
            idf INTEGER NOT NULL DEFAULT 1
        );
        CREATE INDEX IF NOT EXISTS idx_tv_project_token ON token_vectors(project, token);"
    )?;
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════
// Store 方法
// ═══════════════════════════════════════════════════════════════════

impl super::Store {
    /// 插入节点向量（int8 BLOB）
    pub fn upsert_node_vector(&self, node_id: i64, project: &str, vec_f32: &[f32]) -> Result<(), super::StoreError> {
        let blob = quantize_i8(vec_f32);
        self.conn.execute(
            "INSERT OR REPLACE INTO node_vectors (node_id, project, vector) VALUES (?1, ?2, ?3)",
            params![node_id, project, blob],
        )?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════
// 搜索结果
// ═══════════════════════════════════════════════════════════════════

pub struct VectorSearchResult {
    pub node_id: i64,
    pub score: f64,
}

// ═══════════════════════════════════════════════════════════════════
// 稀疏随机投影回退（OOV 关键词）
// 对齐 C 版 vs_fill_sparse_random
// ═══════════════════════════════════════════════════════════════════

/// 对不在 token_vectors 表中的关键词生成稀疏随机向量
/// 用字符串 hash 作为种子，在 VEC_DIM 中设置 8 个非零位
pub fn sparse_random(token: &str) -> Vec<f32> {
    use std::hash::{Hash, Hasher};
    let mut s = std::collections::hash_map::DefaultHasher::new();
    token.hash(&mut s);
    let seed = s.finish();

    let mut vec = vec![0.0f32; VEC_DIM];
    for i in 0..8 {
        let h = seed.wrapping_add(i as u64).wrapping_mul(0x9E3779B97F4A7C15);
        let pos = (h as usize) % VEC_DIM;
        vec[pos] = if (h & 1) == 1 { 1.0 } else { -1.0 };
    }
    vec
}

// ═══════════════════════════════════════════════════════════════════
// 关键词向量构建
// 对齐 C 版 vs_build_keyword_vectors
// ═══════════════════════════════════════════════════════════════════

/// 构建单个关键词的查询向量
/// 1. 查 token_vectors 表取 enriched vector
/// 2. 查不到则用 sparse_random 回退
/// 3. 归一化 + int8 量化
pub fn build_keyword_vector(conn: &Connection, project: &str, keyword: &str) -> Result<Option<Vec<u8>>, rusqlite::Error> {
    // 尝试从 token_vectors 查 enriched vector
    let enriched = conn.query_row(
        "SELECT vector FROM token_vectors WHERE project = ?1 AND token = ?2 LIMIT 1",
        params![project, keyword],
        |row| row.get::<_, Vec<u8>>(0),
    );

    let float_vec = match enriched {
        Ok(blob) => dequantize_f32(&blob),
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            // 回退：稀疏随机投影
            sparse_random(keyword)
        }
        Err(e) => return Err(e),
    };

    // 归一化 + int8 量化
    let mag: f32 = float_vec.iter().map(|v| v * v).sum::<f32>().sqrt();
    if mag < 1e-10 {
        return Ok(None);
    }
    let inv = 1.0 / mag;
    let normalized: Vec<f32> = float_vec.iter().map(|v| v * inv).collect();
    let blob = quantize_i8(&normalized);
    Ok(Some(blob))
}

// ═══════════════════════════════════════════════════════════════════
// min 余弦评分（多关键词）
// 对齐 C 版 vs_min_cosine_score
// ═══════════════════════════════════════════════════════════════════

/// 计算节点向量与多个关键词向量的最小余弦相似度
/// 取 min 而非 avg：确保 ALL 关键词都相关，而非平均相关
pub fn min_cosine_score(node_blob: &[u8], kw_blobs: &[Vec<u8>]) -> f64 {
    let mut min_score = 1.0_f64;
    for kw in kw_blobs {
        if kw.len() != node_blob.len() || node_blob.is_empty() {
            continue;
        }
        let mut dot = 0_i32;
        let mut ma = 0_i32;
        let mut mb = 0_i32;
        for i in 0..node_blob.len() {
            let va = node_blob[i] as i8 as i32;
            let vb = kw[i] as i8 as i32;
            dot += va * vb;
            ma += va * va;
            mb += vb * vb;
        }
        let denom = ((ma as f64).sqrt() * (mb as f64).sqrt()).max(1e-10);
        let cos = dot as f64 / denom;
        if cos < min_score {
            min_score = cos;
        }
    }
    min_score
}

// ═══════════════════════════════════════════════════════════════════
// 向量搜索（多关键词版）
// 对齐 C 版 cbm_store_vector_search
// ═══════════════════════════════════════════════════════════════════

/// 向量搜索 — 多关键词，min 评分
pub fn vector_search(
    conn: &Connection,
    project: &str,
    keywords: &[&str],
    limit: usize,
) -> Result<Vec<VectorSearchResult>, rusqlite::Error> {
    if keywords.is_empty() {
        return Ok(Vec::new());
    }

    // 构建每个关键词的查询向量
    let mut kw_blobs = Vec::new();
    for kw in keywords {
        if let Some(blob) = build_keyword_vector(conn, project, kw)? {
            kw_blobs.push(blob);
        }
    }

    if kw_blobs.is_empty() {
        return Ok(Vec::new());
    }

    // 用第一个关键词做 SQL 预过滤，再在 Rust 层算 min 分
    // 这样能利用 SQLite 索引，同时准确评分
    let first_blob = &kw_blobs[0];
    let mut stmt = conn.prepare(
        "SELECT v.node_id, v.vector
         FROM node_vectors v
         WHERE v.project = ?1
         ORDER BY cbm_cosine_i8(v.vector, ?2) DESC
         LIMIT ?3"
    )?;

    let candidates: Vec<(i64, Vec<u8>)> = stmt.query_map(
        params![project, first_blob, (limit as i64) * 3], // 多取一些，Rust 层重排序
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Vec<u8>>(1)?,
            ))
        },
    )?.collect::<Result<_, _>>()?;

    // Rust 层：算所有关键词的 min 分，重排序
    let mut results: Vec<VectorSearchResult> = candidates
        .into_iter()
        .map(|(node_id, node_vec)| {
            let score = if kw_blobs.len() > 1 {
                min_cosine_score(&node_vec, &kw_blobs)
            } else {
                // 单关键词：直接用 SQLite 算的分数
                // 这里简单处理：用第一个关键词 blob 重算一次
                let mut dot = 0_i32;
                let mut ma = 0_i32;
                let mut mb = 0_i32;
                for i in 0..node_vec.len().min(first_blob.len()) {
                    let va = node_vec[i] as i8 as i32;
                    let vb = first_blob[i] as i8 as i32;
                    dot += va * vb;
                    ma += va * va;
                    mb += vb * vb;
                }
                let denom = ((ma as f64).sqrt() * (mb as f64).sqrt()).max(1e-10);
                dot as f64 / denom
            };
            VectorSearchResult { node_id, score }
        })
        .collect();

    // 按分数降序排列
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(limit);

    Ok(results)
}
