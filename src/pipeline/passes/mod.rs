//! Pipeline 的各个 pass 实现
//!
//! 每个 pass 是一个 struct，实现了 PipelinePass trait。
//! 写新的 pass 就是：定义一个 struct + 实现 PipelinePass。

pub mod discover;
pub mod parse_chapter;
