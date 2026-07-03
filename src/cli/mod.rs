//! 命令行入口 — 使用 clap 定义子命令
//!
//! # 学习重点
//! - `#[derive(Parser)]` — clap 的派生宏
//! - `#[command(name = "...")]` 配置命令元数据
//! - `#[arg(long, short)]` 定义命令行参数
//! - `enum` 作为子命令分发（非常 Rust 的风格）
//! - 先导 `use` 和模块重导出

use clap::{Parser, Subcommand};

// ============================================================
// 顶层 CLI 定义
// ============================================================

/// novelbase-memory-mcp — 小说创作知识图谱 MCP 服务器
///
/// 管理角色、地点、情节线、时间线，为 AI 写作助手提供结构化知识。
///
/// 用法:
///   novelbase-memory-mcp server         启动 MCP 服务
///   novelbase-memory-mcp cli <command>   执行单次命令
///   novelbase-memory-mcp --version      查看版本
///
/// 💡 `#[derive(Parser)]` 是 clap 的魔法
///    它会自动生成命令行参数解析代码
///    `about` 就是 `--help` 时显示的描述
#[derive(Parser, Debug)]
#[command(name = "novelbase-memory-mcp")]
#[command(about = "小说创作知识图谱 MCP 服务器", long_about = None)]
pub struct Cli {
    /// 子命令
    ///
    /// 💡 `Subcommand` 是 clap 的 enum 子命令系统
    ///    每个变体 = 一个子命令
    #[command(subcommand)]
    pub command: Commands,
}

// ============================================================
// 子命令枚举
// ============================================================

/// 所有支持的子命令
///
/// 💡 Rust 的 enum 和 clap 配合：
/// - `Server` 对应 `novelbase-memory-mcp server`
/// - `Cli { ... }` 对应 `novelbase-memory-mcp cli ...`
///
/// clap 会自动从 enum 变体名推导命令名（驼峰 → 小写）
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// 启动 MCP 服务器（stdin/stdout JSON-RPC）
    Server,
    /// 启动 Web UI（浏览器可视化界面）
    Ui {
        /// 端口号
        #[arg(long, default_value = "8080")]
        port: u16,
        /// 项目名（默认 "default"）
        #[arg(long)]
        project: Option<String>,
    },
    /// CLI 模式：执行单次操作
    #[command(subcommand)]
    Cli(CliCommand),
    /// 初始化新小说项目
    Init {
        /// 项目名
        name: String,
        /// 项目路径（默认当前目录）
        #[arg(long, short)]
        path: Option<String>,
    },
    /// 显示配置信息
    Config {
        /// 获取/设置/列出配置项
        #[command(subcommand)]
        action: Option<ConfigAction>,
    },
}

// ============================================================
// 小说管理子命令
// ============================================================

/// CLI 模式下的操作（对应 MCP 工具的 CLI 版）
#[derive(Debug, Subcommand)]
pub enum CliCommand {
    /// 添加角色
    AddCharacter {
        /// 角色名
        name: String,
        /// 项目（默认当前目录项目）
        #[arg(long)]
        project: Option<String>,
        /// 角色特质
        #[arg(long)]
        traits: Option<String>,
    },
    /// 添加关系
    AddRelationship {
        /// 角色A
        character_a: String,
        /// 角色B
        character_b: String,
        /// 关系类型（knows/located_in/appears_in/leads_to/part_of/mentions）
        #[arg(long)]
        relationship_type: String,
        /// 项目
        #[arg(long)]
        project: Option<String>,
    },
    /// 列出角色
    ListCharacters {
        /// 项目
        project: Option<String>,
    },
    /// 搜索图谱（关键词可选，不指定则按 --label 列出所有）
    Search {
        /// 搜索关键词（模糊匹配）
        query: Option<String>,
        /// 节点标签过滤
        #[arg(long)]
        label: Option<String>,
        /// 项目
        #[arg(long)]
        project: Option<String>,
    },
    /// 导入小说文件
    Import {
        /// 文件或目录路径
        path: String,
        /// 项目名
        #[arg(long)]
        project: Option<String>,
    },
}

/// 配置操作
#[derive(Debug, Subcommand)]
pub enum ConfigAction {
    /// 列出所有配置
    List,
    /// 获取配置项
    Get { key: String },
    /// 设置配置项
    Set { key: String, value: String },
}
