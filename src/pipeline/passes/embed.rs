//! EmbeddingPass — 生成节点向量（语义搜索）
//!
//! 对齐 C 版：
//!   1. TextEmbedding（fastembed）生成 f32 向量
//!   2. int8 量化压缩后存入 node_vectors 表
//!   3. 后续查询时先用 cbm_cosine_i8 SQL 函数预过滤
//!      再用 min_cosine_score 多关键词重排序

use crate::pipeline::{Context, PipelinePass, PipelineError};
use crate::store::vectors;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

pub struct EmbeddingPass;

impl PipelinePass for EmbeddingPass {
    fn name(&self) -> &'static str {
        "embedding"
    }

    fn run(&self, ctx: &mut Context) -> Result<(), PipelineError> {
        // 收集需要生成向量的节点
        let nodes: Vec<_> = ctx
            .graph
            .find_by_label(crate::models::NodeLabel::Character)
            .into_iter()
            .chain(ctx.graph.find_by_label(crate::models::NodeLabel::Location))
            .cloned()
            .collect();

        if nodes.is_empty() {
            println!("  Embedding: 无节点");
            return Ok(());
        }

        println!("  Embedding: 初始化模型（首次运行自动下载 ~30MB）...");

        let model = match TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::BGESmallZHV15)
                .with_show_download_progress(true),
        ) {
            Ok(m) => m,
            Err(e) => {
                println!("  跳过 EmbeddingPass（模型下载失败，需要网络）: {}", e);
                return Ok(());
            }
        };

        let mut count = 0;
        for node in &nodes {
            // 构建输入文本：角色名 + 关键属性
            let text = if node.label == crate::models::NodeLabel::Character {
                format!(
                    "{}。{}。{}。{}。{}",
                    node.name,
                    node.properties.get_str("identity").unwrap_or(""),
                    node.properties.get_str("personality").unwrap_or(""),
                    node.properties.get_str("motivation").unwrap_or(""),
                    node.properties.get_str("fate").unwrap_or(""),
                )
            } else {
                format!(
                    "{}。{}",
                    node.name,
                    node.properties.get_str("description").unwrap_or("")
                )
            };

            if text.trim().is_empty() {
                continue;
            }

            match model.embed(vec![text], Some(1)) {
                Ok(embeddings) => {
                    if let Some(vec) = embeddings.into_iter().next() {
                        // int8 量化后存储
                        if let Err(e) =
                            ctx.store.upsert_node_vector(node.id, &node.project, &vec)
                        {
                            eprintln!("  存储失败 {}: {}", node.name, e);
                            continue;
                        }
                        count += 1;
                    }
                }
                Err(e) => eprintln!("  生成失败 {}: {}", node.name, e),
            }
        }

        println!("  Embedding: 生成 {} 个向量（{} 维，int8 量化）", count, vectors::VEC_DIM);
        Ok(())
    }
}
