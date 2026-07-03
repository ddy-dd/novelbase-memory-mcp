//! novelbase-memory-mcp — 小说创作知识图谱 MCP 服务器
//!
//! 受 codebase-memory-mcp 启发，为小说创作提供结构化知识管理。
//! 管理角色、地点、场景、情节线和时间线，通过 MCP 协议为 AI 写作助手赋能。
//!
//! # 学习重点（整体项目）
//! 这个文件是整个程序的**入口**，它的工作很简单：
//! 1. 解析命令行参数
//! 2. 根据子命令调用对应的处理逻辑
//!
//! 具体的业务逻辑在其他模块（models、store、mcp、pipeline）里。
//! 这种"入口只负责调度"的模式叫**关注点分离**。

// ============================================================
// 模块声明
// ============================================================

// 💡 Rust 的模块系统：
// `mod cli;` 告诉编译器加载 `src/cli/mod.rs` 或 `src/cli.rs`
// `mod models;` 告诉编译器加载 `src/models/mod.rs`
//
// `pub use ...` 重导出，让外部代码可以直接用 `models::Node` 而不是 `models::node::Node`
// 但目前 main.rs 是顶层入口，不需要 pub，用 use 就够了

mod cli;
mod mcp;
mod models;
mod pipeline;
pub mod store;
mod ui;
// ============================================================
// 导入
// ============================================================

// 💡 `use` 语句引入其他模块的公共项
use clap::Parser;
use cli::{Cli, CliCommand, Commands, ConfigAction};
use log::info;
use models::{Edge, EdgeType, Node, NodeLabel};
use store::Store;

// ============================================================
// 主入口
// ============================================================

/// 默认数据库路径
const DB_PATH: &str = "novelbase.db";

/// 把 StoreError 转成 anyhow::Error
///
/// 💡 为什么需要这个？
///    StoreError 里包裹了 rusqlite::Error，它没有实现 Sync trait
///    所以 ? 不能直接把 StoreError 转成 anyhow::Error
///    anyhow! 宏把错误转成字符串存起来，绕过了这个限制
fn store_ok<T>(r: Result<T, store::StoreError>) -> anyhow::Result<T> {
    r.map_err(|e| anyhow::anyhow!("{:#}", e))
}

fn main() -> anyhow::Result<()> {
    // 初始化日志（从 RUST_LOG 环境变量读取日志级别）
    // 用法: `RUST_LOG=info cargo run -- server`
    env_logger::init();

    // 💡 clap 的 `Parser::parse()` 自动解析命令行参数
    // 如果参数不合法，自动打印错误并退出
    let cli = Cli::parse();

    info!("novelbase-memory-mcp 启动");

    // 💡 match 是 Rust 的"瑞士军刀"——它是表达式（可以返回值）
    // 编译器会检查是否穷举了所有变体
    match cli.command {
        Commands::Server => run_server(),
        Commands::Ui { port, project } => run_ui(port, project.as_deref()),
        Commands::Cli(cmd) => run_cli_command(cmd),
        Commands::Init { name, path } => init_project(&name, path.as_deref()),
        Commands::Config { action } => handle_config(action),
    }
}

// ============================================================
// 命令处理函数
// ============================================================

/// 启动 MCP 服务器模式
///
/// 💡 这个函数目前是占位符——后续会实现在 `mcp::Server` 里
///    `todo!()` 宏表示"还没写，编译能过，运行时崩"
///    这是 Rust 开发常用的"先搭架子"方式
fn run_server() -> anyhow::Result<()> {
    info!("MCP 服务器模式");
    let store = store_ok(Store::open(DB_PATH))?;
    let server = mcp::Server::new(store);
    server.run()?;
    Ok(())
}

/// 启动 Web UI
fn run_ui(port: u16, _project: Option<&str>) -> anyhow::Result<()> {
    info!("Web UI 模式 (端口 {})", port);
    let rt = tokio::runtime::Runtime::new()?;
    let server = ui::UIServer::new(DB_PATH);
    rt.block_on(server.run(port))?;
    Ok(())
}

/// 处理 CLI 命令
///
/// 💡 这里的 match 嵌套了 `CliCommand` 的变体
///    Rust 允许 enum 嵌套，C 没有这个能力
fn run_cli_command(cmd: CliCommand) -> anyhow::Result<()> {
    match cmd {
        CliCommand::AddCharacter { name, project, traits } => {
            add_character(&name, project.as_deref(), traits.as_deref())
        }
        CliCommand::AddRelationship { character_a, character_b, relationship_type, project } => {
            add_relationship(&character_a, &character_b, &relationship_type, project.as_deref())
        }
        CliCommand::ListCharacters { project } => list_characters(project.as_deref()),
        CliCommand::Search { query, label, project } => {
            search_graph(query.as_deref(), label.as_deref(), project.as_deref())
        }
        CliCommand::Import { path, project, source } => {
            import_file(&path, project.as_deref(), source.as_deref())
        }
    }
}

// ============================================================
// 具体业务逻辑（目前是占位符）
// ============================================================

/// 添加角色
fn add_character(name: &str, project: Option<&str>, traits: Option<&str>) -> anyhow::Result<()> {
    let project = project.unwrap_or("default");
    let store = store_ok(Store::open(DB_PATH))?;

    let mut node = Node::new(project, NodeLabel::Character, name, &format!("{}.{}", project, name));
    if let Some(t) = traits {
        node.properties.insert("traits", t);
    }

    let id = store_ok(store.upsert_node(&node))?;
    println!("✅ 角色 '{}' 已添加，ID: {}", name, id);
    Ok(())
}

/// 添加关系
fn add_relationship(
    character_a: &str,
    character_b: &str,
    relationship_type: &str,
    project: Option<&str>,
) -> anyhow::Result<()> {
    let project = project.unwrap_or("default");
    let store = store_ok(Store::open(DB_PATH))?;

    let a_qn = format!("{}.{}", project, character_a);
    let b_qn = format!("{}.{}", project, character_b);

    let a = store_ok(store.find_node_by_qn(project, &a_qn))?
        .ok_or_else(|| anyhow::anyhow!("未找到角色: {}", character_a))?;
    let b = store_ok(store.find_node_by_qn(project, &b_qn))?
        .ok_or_else(|| anyhow::anyhow!("未找到角色: {}", character_b))?;

    let edge_type = EdgeType::from_str(relationship_type)
        .ok_or_else(|| anyhow::anyhow!("未知关系类型: {}", relationship_type))?;

    let edge = Edge::new(project, a.id, b.id, edge_type.clone());
    let id = store_ok(store.insert_edge(&edge))?;
    println!("✅ 关系已添加 (ID: {}) — {} --[{}]--> {}", id, character_a, edge_type.as_str(), character_b);
    Ok(())
}

/// 列出角色
fn list_characters(project: Option<&str>) -> anyhow::Result<()> {
    let project = project.unwrap_or("default");
    let store = store_ok(Store::open(DB_PATH))?;

    let characters = store_ok(store.find_nodes_by_label(project, NodeLabel::Character))?;

    println!("📋 角色列表（项目: {}）:", project);
    for c in &characters {
        println!("  - {} (ID: {})", c.name, c.id);
    }
    println!("共 {} 个角色", characters.len());
    Ok(())
}

/// 搜索图谱
///
/// query 参数：按名字模糊搜索（LIKE %query%）
/// --label 参数：按标签过滤（可选）
/// 两个都提供 = 搜名字 + 过滤标签
/// 只提供 query = 搜所有类型节点
/// 只提供 --label = 列出该类型所有节点
fn search_graph(query: Option<&str>, label: Option<&str>, project: Option<&str>) -> anyhow::Result<()> {
    let project = project.unwrap_or("default");
    let store = store_ok(Store::open(DB_PATH))?;

    let nodes = if let Some(q) = query {
        // 有关键词 → 优先用 FTS5 全文搜索（相关度排序，支持中文）
        let mut nodes = match store_ok(store.search_fts(project, q)) {
            Ok(n) if !n.is_empty() => n,
            // FTS 搜不到或报错 → 回退到 LIKE 模糊匹配
            _ => store_ok(store.find_nodes_by_name(project, q))?,
        };
        // 如果还指定了 label，在 Rust 层过滤
        if let Some(label_str) = label {
            match NodeLabel::from_str(label_str) {
                Some(node_label) => {
                    nodes.retain(|n| n.label == node_label);
                }
                None => {
                    println!("未知标签: {}，可用: character/location/scene/chapter/plotline/timeline/item/note/project/file", label_str);
                    return Ok(());
                }
            }
        }
        nodes
    } else if let Some(label_str) = label {
        // 没关键词但指定了 label → 列出该类型所有节点
        match NodeLabel::from_str(label_str) {
            Some(node_label) => {
                store_ok(store.find_nodes_by_label(project, node_label))?
            }
            None => {
                println!("未知标签: {}", label_str);
                return Ok(());
            }
        }
    } else {
        println!("请指定搜索关键词或 --label");
        return Ok(());
    };

    if nodes.is_empty() {
        println!("没有找到匹配的节点");
        return Ok(());
    }

    println!("📋 找到 {} 个节点:", nodes.len());
    for n in &nodes {
        println!("  - [{}] {} (ID: {}, 文件: {})",
            n.label.cn_name(),
            n.name,
            n.id,
            n.file_path.as_deref().unwrap_or("-"),
        );
    }
    Ok(())
}

/// 导入文件/目录到知识图谱
///
/// 用 pipeline 扫描 .md 文件，提取节点信息存入数据库
fn import_file(path: &str, project: Option<&str>, source: Option<&str>) -> anyhow::Result<()> {
    // 确定项目名（没指定就从目录名推断）
    let project_name = match project {
        Some(name) => name.to_string(),
        None => std::path::Path::new(path)
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("default")
            .to_string(),
    };

    // 确定扫描目录（如果是文件，用其所在目录）
    let path_buf = std::path::Path::new(path);
    let repo_path = if path_buf.is_file() {
        path_buf.parent().unwrap_or(std::path::Path::new("."))
    } else {
        path_buf
    };
    let repo_path_str = repo_path.to_string_lossy().to_string();

    // 打开数据库，确保项目存在
    let store = store_ok(Store::open(DB_PATH))?;
    store_ok(store.ensure_project(&project_name))?;

    // 创建 Pipeline 并运行
    let mut graph = pipeline::graph_buf::GraphBuffer::new(&project_name);
    let cancelled = std::sync::atomic::AtomicBool::new(false);

    // 💡 大括号创建一个作用域——pipeline 跑完后 ctx 自动释放
    //    这样 ctx 借用的 &mut graph 才会归还，后面才能调用 graph.dump_to_store
    {
        let source_str = source.unwrap_or("original");
        let mut ctx = pipeline::context::Context::new(
            &project_name,
            &repo_path_str,
            &store,
            &mut graph,
            &cancelled,
            source_str,
        );

        let pipeline = pipeline::Pipeline::new(vec![
            Box::new(pipeline::passes::discover::DiscoverPass),
            Box::new(pipeline::passes::parse_chapter::ParseChapterPass),
            Box::new(pipeline::passes::embed::EmbeddingPass),
        ]);

        pipeline.run(&mut ctx)?;
    } // 👈 ctx 在这里被 drop，graph 的借用结束

    // 把内存中的图数据刷到 SQLite
    // 💡 dump_to_store 会更新 graph 里 node 的 ID（临时 ID → SQLite 真实 ID）
    graph.dump_to_store(&store)?;

    println!(
        "✅ 导入完成: 项目 '{}'，{} 个节点，{} 条边",
        project_name,
        graph.node_count(),
        graph.edge_count()
    );
    Ok(())
}

/// 初始化项目
fn init_project(name: &str, path: Option<&str>) -> anyhow::Result<()> {
    let db_path = match path {
        Some(p) => format!("{}/novelbase.db", p.trim_end_matches('/')),
        None => DB_PATH.to_string(),
    };
    let store = store_ok(Store::open(&db_path))?;
    store_ok(store.ensure_project(name))?;
    println!("✅ 项目 '{}' 已初始化，数据库: {}", name, db_path);
    Ok(())
}

/// 处理配置
fn handle_config(action: Option<ConfigAction>) -> anyhow::Result<()> {
    match action {
        Some(ConfigAction::List) => {
            println!("配置列表（暂未实现）");
        }
        Some(ConfigAction::Get { key }) => {
            println!("获取配置: {}（暂未实现）", key);
        }
        Some(ConfigAction::Set { key, value }) => {
            println!("设置配置: {} = {}（暂未实现）", key, value);
        }
        None => {
            println!("运行 `novelbase-memory-mcp config --help` 查看配置用法");
        }
    }
    Ok(())
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parse_server() {
        // 💡 clap 的 `try_parse_from` 直接测试参数解析
        let cli = Cli::try_parse_from(["novelbase", "server"]).unwrap();
        assert!(matches!(cli.command, Commands::Server));
    }

    #[test]
    fn test_cli_parse_add_character() {
        let cli = Cli::try_parse_from(["novelbase", "cli", "add-character", "张三"]).unwrap();
        match cli.command {
            Commands::Cli(CliCommand::AddCharacter { name, .. }) => {
                assert_eq!(name, "张三");
            }
            _ => panic!("期望 AddCharacter 命令"),
        }
    }

    #[test]
    fn test_cli_parse_init() {
        let cli = Cli::try_parse_from(["novelbase", "init", "我的小说"]).unwrap();
        match cli.command {
            Commands::Init { name, .. } => {
                assert_eq!(name, "我的小说");
            }
            _ => panic!("期望 Init 命令"),
        }
    }
}
