//! Pipeline — 流水线编排器
//!
//! 把小说文件导入知识图谱时，需要多个步骤：
//! 1. 发现文件（扫描目录）→ discover pass
//! 2. 解析内容（提取角色/场景）→ parse pass
//! 3. 推导关系（谁认识谁）→ relate pass
//!
//! 每个步骤是一个 pass，它们都实现了同一个 trait（接口）。
//! Pipeline 把 pass 串起来逐个执行。
//!
//! # 学习重点
//! - `trait` — Rust 的"接口"，定义行为
//! - `impl Trait for Struct` — 为类型实现 trait
//! - `Vec<Box<dyn Trait>>` — 动态分发（类似 C 的函数指针数组）
//! - `&mut Context` — 可变引用传递上下文

pub mod context;
pub mod graph_buf;
pub mod passes;

use context::Context;
use std::sync::atomic::Ordering;

// ============================================================
// Pipeline 错误类型
// ============================================================

/// Pipeline 自己的错误类型
#[derive(Debug)]
pub enum PipelineError {
    /// I/O 错误（读文件时）
    Io(std::io::Error),
    /// 存储错误
    Store(crate::store::StoreError),
    /// 用户取消
    Cancelled,
    /// 其他错误
    Other(String),
}

// 让 ? 可以直接用 std::io::Error
impl From<std::io::Error> for PipelineError {
    fn from(e: std::io::Error) -> Self {
        PipelineError::Io(e)
    }
}

// 让 ? 可以直接用 StoreError
impl From<crate::store::StoreError> for PipelineError {
    fn from(e: crate::store::StoreError) -> Self {
        PipelineError::Store(e)
    }
}

// 实现 Display（让 PipelineError 可以被打印）
impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineError::Io(e) => write!(f, "I/O 错误: {}", e),
            PipelineError::Store(e) => write!(f, "存储错误: {}", e),
            PipelineError::Cancelled => write!(f, "用户取消"),
            PipelineError::Other(msg) => write!(f, "错误: {}", msg),
        }
    }
}

// 实现 std::error::Error（让 anyhow::Error 可以自动转换）
impl std::error::Error for PipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PipelineError::Io(e) => Some(e),
            PipelineError::Store(e) => Some(e),
            PipelineError::Cancelled | PipelineError::Other(_) => None,
        }
    }
}

// ============================================================
// PipelinePass trait — 每个 pass 必须实现
// ============================================================

/// 一个 pipeline pass 必须实现的行为
///
/// 💡 trait 就是 Rust 的"接口"
///    C 版：函数指针数组 `pass_func_t passes[]`
///    Rust：`trait PipelinePass { fn run(&self, ctx: &mut Context) -> ... }`
///
/// 区别：
/// - C 的函数指针只管函数签名，不管行为保证
/// - Rust 的 trait 可以保证每个实现了它的类型都有 run() 方法
pub trait PipelinePass {
    /// pass 的名字（用于日志和调试）
    fn name(&self) -> &'static str;

    /// 执行 pass
    ///
    /// 💡 `&mut Context` 表示 pass 可以修改上下文
    ///    可以往 graph 里加节点/边，也可以读 store
    fn run(&self, ctx: &mut Context) -> Result<(), PipelineError>;
}

// ============================================================
// Pipeline 编排器
// ============================================================

/// Pipeline 编排器 — 按顺序执行多个 pass
///
/// 💡 泛型参数 P: PipelinePass
///    意思是 P 可以是任何实现了 PipelinePass 的类型
///    这里用 `Box<dyn PipelinePass>` 实现动态分发
pub struct Pipeline {
    /// pass 列表（按顺序执行）
    passes: Vec<Box<dyn PipelinePass>>,
}

impl Pipeline {
    /// 创建新的 Pipeline
    pub fn new(passes: Vec<Box<dyn PipelinePass>>) -> Self {
        Self { passes }
    }

    /// 执行所有 pass
    ///
    /// 💡 按顺序逐个调用 pass.run()
    ///    每个 pass 执行前检查取消标志
    pub fn run(&self, ctx: &mut Context) -> Result<(), PipelineError> {
        for pass in &self.passes {
            // 检查是否被取消
            if ctx.cancelled.load(Ordering::Relaxed) {
                return Err(PipelineError::Cancelled);
            }

            println!("⏳ 执行 pass: {}", pass.name());
            pass.run(ctx)?;
            println!("✅ pass 完成: {}", pass.name());
        }
        println!("🎉 Pipeline 全部完成");
        Ok(())
    }
}
