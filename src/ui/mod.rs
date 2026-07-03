//! Web UI — 浏览器中展示知识图谱的力导向图
//!
//! 启动一个 HTTP 服务器，在浏览器打开后能看到：
//! - 角色、文件、地点等节点（不同颜色）
//! - 节点之间的关系连线
//! - 拖拽、缩放、搜索功能

use axum::{
    extract::Query,
    response::Html,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use crate::store::Store;

// ============================================================
// API 数据结构（D3.js 格式）
// ============================================================

/// 图的节点（给 D3.js 用）
#[derive(Serialize)]
struct GraphNode {
    id: i64,
    name: String,
    label: String,   // "Character", "File" 等
    group: String,   // "character", "file" — 用于前端着色
}

/// 图的边（给 D3.js 用）
#[derive(Serialize)]
struct GraphLink {
    source: i64,
    target: i64,
    edge_type: String,
}

/// API 返回的完整图数据
#[derive(Serialize)]
struct GraphData {
    nodes: Vec<GraphNode>,
    links: Vec<GraphLink>,
}

/// 查询参数
#[derive(serde::Deserialize)]
struct ProjectParam {
    project: Option<String>,
}

// ============================================================
// 服务器
// ============================================================

/// UI 服务器
pub struct UIServer {
    db_path: String,
}

impl UIServer {
    pub fn new(db_path: &str) -> Self {
        Self {
            db_path: db_path.to_string(),
        }
    }

    /// 启动 HTTP 服务器（阻塞，直到按 Ctrl+C）
    pub async fn run(&self, port: u16) -> anyhow::Result<()> {
        let db_path = Arc::new(self.db_path.clone());

        let app = Router::new()
            .route("/", get(index_html))
            .route("/api/graph", get({
                let db_path = db_path.clone();
                move |query| graph_handler(query, db_path.clone())
            }))
            .route("/api/search", get({
                let db_path = db_path.clone();
                move |query| search_handler(query, db_path.clone())
            }));

        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        println!("🌐 打开浏览器访问: http://localhost:{}/", port);
        println!("   按 Ctrl+C 停止服务器");

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}

// ============================================================
// API 处理器
// ============================================================

/// Graph API：返回项目的所有节点和边（JSON 格式）
async fn graph_handler(
    Query(params): Query<ProjectParam>,
    db_path: Arc<String>,
) -> Json<GraphData> {
    let project = params.project.as_deref().unwrap_or("default");

    // 每个请求新建一个只读连接
    let store = match Store::open(db_path.as_str()) {
        Ok(s) => s,
        Err(_) => return Json(GraphData { nodes: vec![], links: vec![] }),
    };

    // 收集所有节点
    let all_labels = [
        crate::models::NodeLabel::Character,
        crate::models::NodeLabel::Location,
        crate::models::NodeLabel::File,
        crate::models::NodeLabel::Scene,
        crate::models::NodeLabel::Item,
        crate::models::NodeLabel::Note,
    ];

    let mut nodes = Vec::new();
    for label in all_labels {
        if let Ok(ns) = store.find_nodes_by_label(project, label) {
            for n in ns {
                let group = match n.label {
                    crate::models::NodeLabel::Character => "character",
                    crate::models::NodeLabel::Location => "location",
                    crate::models::NodeLabel::File => "file",
                    crate::models::NodeLabel::Scene => "scene",
                    crate::models::NodeLabel::Item => "item",
                    _ => "other",
                };
                nodes.push(GraphNode {
                    id: n.id,
                    name: n.name,
                    label: n.label.as_str().to_string(),
                    group: group.to_string(),
                });
            }
        }
    }

    // 收集所有边
    let mut links = Vec::new();
    if let Ok(edges) = store.find_all_edges(project) {
        for e in edges {
            links.push(GraphLink {
                source: e.source_id,
                target: e.target_id,
                edge_type: e.edge_type.as_str().to_string(),
            });
        }
    }

    Json(GraphData { nodes, links })
}

/// Search API：搜索节点（JSON 格式）
async fn search_handler(
    _params: Query<ProjectParam>,
    _db_path: Arc<String>,
) -> Json<GraphData> {
    let _store = match Store::open(_db_path.as_str()) {
        Ok(s) => s,
        Err(_) => return Json(GraphData { nodes: vec![], links: vec![] }),
    };

    Json(GraphData { nodes: vec![], links: vec![] })
}

// ============================================================
// 嵌入的 HTML 页面（D3.js 力导向图）
// ============================================================

/// 返回完整的 HTML 页面
async fn index_html() -> Html<&'static str> {
    Html(HTML_PAGE)
}

const HTML_PAGE: &str = r###"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<title>novelbase 知识图谱</title>
<script src="https://d3js.org/d3.v7.min.js"></script>
<style>
* { margin: 0; padding: 0; box-sizing: border-box; }
body { font-family: -apple-system, "PingFang SC", "Microsoft YaHei", sans-serif; background: #1a1a2e; color: #eee; overflow: hidden; }

#toolbar {
    position: fixed; top: 0; left: 0; right: 0; z-index: 10;
    background: rgba(26,26,46,0.9); backdrop-filter: blur(8px);
    padding: 12px 20px; display: flex; align-items: center; gap: 12px;
    border-bottom: 1px solid #333;
}
#toolbar h1 { font-size: 16px; font-weight: 600; color: #e94560; }
#toolbar input {
    padding: 6px 12px; border-radius: 6px; border: 1px solid #444;
    background: #16213e; color: #eee; font-size: 14px; width: 200px; outline: none;
}
#toolbar input:focus { border-color: #e94560; }
#toolbar select {
    padding: 6px 12px; border-radius: 6px; border: 1px solid #444;
    background: #16213e; color: #eee; font-size: 14px; outline: none;
}

#legend {
    position: fixed; bottom: 20px; right: 20px; z-index: 10;
    background: rgba(22,33,62,0.9); padding: 10px 14px; border-radius: 8px;
    font-size: 12px; border: 1px solid #333;
}
#legend .item { display: flex; align-items: center; gap: 6px; margin: 3px 0; }
#legend .dot { width: 10px; height: 10px; border-radius: 50%; }

#info {
    position: fixed; bottom: 20px; left: 20px; z-index: 10;
    background: rgba(22,33,62,0.9); padding: 10px 14px; border-radius: 8px;
    font-size: 13px; border: 1px solid #333; max-width: 300px; display: none;
}
#info h3 { color: #e94560; margin-bottom: 4px; }
#info p { margin: 2px 0; color: #aaa; }

svg { width: 100vw; height: 100vh; }
</style>
</head>
<body>

<div id="toolbar">
    <h1>📖 novelbase</h1>
    <input id="search" type="text" placeholder="搜索节点..." oninput="onSearch(this.value)">
    <select id="project-select" onchange="loadGraph()">
        <option value="测试小说">测试小说</option>
    </select>
</div>

<div id="legend">
    <div class="item"><span class="dot" style="background:#e94560"></span> 角色</div>
    <div class="item"><span class="dot" style="background:#0f3460"></span> 地点</div>
    <div class="item"><span class="dot" style="background:#16c79a"></span> 文件</div>
    <div class="item"><span class="dot" style="background:#f5a623"></span> 场景</div>
    <div class="item"><span class="dot" style="background:#7b2fbe"></span> 物品</div>
</div>

<div id="info"></div>

<svg id="graph"></svg>

<script>
const COLORS = {
    character: "#e94560", location: "#0f3460", file: "#16c79a",
    scene: "#f5a623", item: "#7b2fbe", other: "#666",
};

let svg = d3.select("#graph");
let width = window.innerWidth;
let height = window.innerHeight;
let simulation, g, link, node, label;

function resize() {
    width = window.innerWidth;
    height = window.innerHeight;
    svg.attr("width", width).attr("height", height);
}
window.onresize = resize;
resize();

g = svg.append("g");

// 缩放
svg.call(d3.zoom().scaleExtent([0.1, 8]).on("zoom", (e) => {
    g.attr("transform", e.transform);
}));

async function loadGraph() {
    const project = document.getElementById("project-select").value;
    const resp = await fetch(`/api/graph?project=${encodeURIComponent(project)}`);
    const data = await resp.json();

    if (data.nodes.length === 0) {
        document.getElementById("info").style.display = "block";
        document.getElementById("info").innerHTML = "<p>⚠️ 暂无数据</p>";
        return;
    }

    // 清除旧图
    g.selectAll("*").remove();
    document.getElementById("info").style.display = "none";

    simulation = d3.forceSimulation(data.nodes)
        .force("link", d3.forceLink(data.links).id(d => d.id).distance(100))
        .force("charge", d3.forceManyBody().strength(-300))
        .force("center", d3.forceCenter(width / 2, height / 2))
        .force("collision", d3.forceCollide(30));

    link = g.append("g").selectAll("line")
        .data(data.links).join("line")
        .attr("stroke", "#555").attr("stroke-width", 1.5)
        .attr("stroke-opacity", 0.6);

    // 箭头标记
    svg.append("defs").selectAll("marker")
        .data(["end"]).join("marker")
        .attr("id", "arrow").attr("viewBox", "0 -5 10 10")
        .attr("refX", 22).attr("refY", 0)
        .attr("markerWidth", 6).attr("markerHeight", 6)
        .attr("orient", "auto")
        .append("path").attr("d", "M0,-5L10,0L0,5").attr("fill", "#555");

    link.attr("marker-end", "url(#arrow)");

    node = g.append("g").selectAll("circle")
        .data(data.nodes).join("circle")
        .attr("r", 8).attr("fill", d => COLORS[d.group] || "#666")
        .attr("stroke", "#fff").attr("stroke-width", 1.5)
        .attr("cursor", "pointer")
        .on("click", (e, d) => showInfo(d))
        .call(d3.drag()
            .on("start", (e, d) => { if (!e.active) simulation.alphaTarget(0.3).restart(); d.fx = d.x; d.fy = d.y; })
            .on("drag", (e, d) => { d.fx = e.x; d.fy = e.y; })
            .on("end", (e, d) => { if (!e.active) simulation.alphaTarget(0); d.fx = null; d.fy = null; })
        );

    label = g.append("g").selectAll("text")
        .data(data.nodes).join("text")
        .text(d => d.name)
        .attr("font-size", "11px").attr("dx", 12).attr("dy", 4)
        .attr("fill", "#aaa").attr("pointer-events", "none");

    simulation.on("tick", () => {
        link.attr("x1", d => d.source.x).attr("y1", d => d.source.y)
            .attr("x2", d => d.target.x).attr("y2", d => d.target.y);
        node.attr("cx", d => d.x).attr("cy", d => d.y);
        label.attr("x", d => d.x).attr("y", d => d.y);
    });
}

function showInfo(d) {
    const info = document.getElementById("info");
    info.style.display = "block";
    info.innerHTML = `<h3>${d.name}</h3><p>类型: ${d.label}</p><p>ID: ${d.id}</p>`;
}

function onSearch(text) {
    if (!node) return;
    node.attr("opacity", d => {
        return !text || d.name.includes(text) || d.label.toLowerCase().includes(text.toLowerCase()) ? 1 : 0.15;
    });
    label.attr("opacity", d => {
        return !text || d.name.includes(text) || d.label.toLowerCase().includes(text.toLowerCase()) ? 1 : 0.15;
    });
}

loadGraph();
</script>
</body>
</html>"###;
