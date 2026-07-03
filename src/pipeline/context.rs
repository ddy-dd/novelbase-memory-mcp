//! Pipeline 上下文 — 每个 pass 共享的数据
//!
//! # 学习重点
//! - 生命周期 `'a` — 告诉编译器 ctx 里的引用能活多久
//! - `&'a Store` — 不可变引用（所有 pass 都可以读 store）
//! - `&'a mut GraphBuffer` — 可变引用（pass 可以往 graph 里写数据）
//! - `&'a AtomicBool` — 线程安全的取消标志

use crate::store::Store;
use crate::pipeline::graph_buf::GraphBuffer;
use std::sync::atomic::AtomicBool;

/// Pipeline 执行上下文
///
/// 💡 生命周期参数 `'a`
///    表示 Context 里的引用至少要活 'a 这么久
///    实际上就是整个 pipeline.run() 的执行期间
///
/// 为什么需要生命周期？
/// - Context 自己不拥有数据，只是"借用"（引用）
/// - 编译器需要确保这些引用在使用时仍然有效
/// - `'a` 就是告诉编译器："这些引用至少和 Context 一样长命"
pub struct Context<'a> {
    /// 项目名（比如"三体"）
    pub project_name: &'a str,
    /// 项目根目录路径
    pub repo_path: &'a str,
    /// 数据库存储（只读）
    pub store: &'a Store,
    /// 内存图缓冲区（读写）
    pub graph: &'a mut GraphBuffer,
    /// 取消标志（设为 true 时 Pipeline 会停止）
    pub cancelled: &'a AtomicBool,
    /// 来源标记（"original" 原著、"continuation" 续写）
    pub source: &'a str,
}

impl<'a> Context<'a> {
    /// 创建新的上下文
    pub fn new(
        project_name: &'a str,
        repo_path: &'a str,
        store: &'a Store,
        graph: &'a mut GraphBuffer,
        cancelled: &'a AtomicBool,
        source: &'a str,
    ) -> Self {
        Self {
            project_name,
            repo_path,
            store,
            graph,
            cancelled,
            source,
        }
    }
}
