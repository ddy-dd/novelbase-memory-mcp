//! DiscoverPass — 发现项目中的小说文件
//!
//! 扫描项目目录，找到所有 .md 文件，计算 hash 对比是否已处理。
//! 没变化的文件跳过（增量导入），新增或修改过的才创建 File 节点。
//!
//! # 学习重点
//! - 为 struct 实现 trait（impl PipelinePass for DiscoverPass）
//! - std::fs::read_dir — 遍历目录
//! - 文件 hash 计算 + 增量检查

use crate::models::{Node, NodeLabel};
use crate::pipeline::{Context, PipelinePass, PipelineError};
use crate::store::file_hashes::FileHash;

/// 文件发现 pass
///
/// 扫描项目目录，找到 .md 文件，对比 file_hashes 表跳过未修改的
/// 对应 C 版的 pass_discover
pub struct DiscoverPass;

impl PipelinePass for DiscoverPass {
    fn name(&self) -> &'static str {
        "discover"
    }

    fn run(&self, ctx: &mut Context) -> Result<(), PipelineError> {
        let path = ctx.repo_path;
        println!("  扫描目录: {}", path);

        // 读取目录
        let entries = std::fs::read_dir(path)?;

        let mut new_count = 0;
        let mut skip_count = 0;
        for entry in entries {
            let entry = entry?;
            let file_path = entry.path();

            // 只处理 .md 文件
            if file_path.extension().map(|e| e == "md").unwrap_or(false) {
                let file_name = file_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown");
                let rel_path = file_path.strip_prefix(path)
                    .unwrap_or(&file_path);
                let rel_str = rel_path.to_string_lossy();

                // 计算文件 hash，对比是否已处理过
                match FileHash::compute(ctx.project_name, &rel_str, &file_path) {
                    Ok(hash) => {
                        // 从数据库查这个文件的旧 hash
                        let existing = ctx.store.get_file_hash(ctx.project_name, &rel_str)?;
                        if let Some(old) = existing {
                            if old.sha256 == hash.sha256 {
                                // 文件没变 → 跳过，但还是要建一个 File 节点
                                let qn = format!("{}.{}", ctx.project_name, rel_str);
                                let mut node = Node::new(ctx.project_name, NodeLabel::File, file_name, &qn)
                                    .with_file(&rel_str);
                                node.properties.insert("source", ctx.source);
                                ctx.graph.upsert_node(node);
                                skip_count += 1;
                                continue;
                            }
                        }

                        // 新文件或已修改 → 创建节点，更新 hash 记录
                        let qn = format!("{}.{}", ctx.project_name, rel_str);
                        let mut node = Node::new(ctx.project_name, NodeLabel::File, file_name, &qn)
                            .with_file(&rel_str);
                        node.properties.insert("source", ctx.source);
                        let id = ctx.graph.upsert_node(node);
                        ctx.store.upsert_file_hash(&hash)?;
                        new_count += 1;
                        println!("  新/修改: {} (ID: {})", file_name, id);
                    }
                    Err(e) => {
                        // 文件读不了就跳过（可能是权限问题）
                        println!("  跳过文件 {}: {}", file_name, e);
                    }
                }
            }
        }

        println!("  结果: {} 个新文件, {} 个跳过", new_count, skip_count);
        Ok(())
    }
}
