//! Web UI — 深色主题 D3.js 力导向图可视化
//!
//! 对齐 codebase-memory-mcp 的视觉风格：
//! - 深空色背景 (#06090f)
//! - 青绿强调色 (#1DA27E)
//! - 毛玻璃侧边栏 + 详情面板
//! - 节点按连接数分大小，按类型分颜色
//! - 边按类型分颜色 + 箭头

use axum::{extract::Query, response::Html, routing::get, Json, Router};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use crate::store::Store;

#[derive(Serialize)]
struct GraphNode { id: i64, name: String, label: String, group: String, props: serde_json::Value }
#[derive(Serialize)]
struct GraphLink { source: i64, target: i64, edge_type: String }
#[derive(Serialize)]
struct GraphData { nodes: Vec<GraphNode>, links: Vec<GraphLink> }
#[derive(serde::Deserialize)]
struct ProjectParam { project: Option<String> }

pub struct UIServer { db_path: String }

impl UIServer {
    pub fn new(db_path: &str) -> Self { Self { db_path: db_path.to_string() } }

    pub async fn run(&self, port: u16) -> anyhow::Result<()> {
        let db_path = Arc::new(self.db_path.clone());
        let app = Router::new()
            .route("/", get(index_html))
            .route("/api/graph", get({
                let p = db_path.clone(); move |q| graph_handler(q, p.clone())
            }))
            .route("/api/projects", get({
                let p = db_path.clone(); move || projects_handler(p.clone())
            }));
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        println!("🌐 打开浏览器: http://localhost:{}/", port);
        println!("   按 Ctrl+C 停止");
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn graph_handler(Query(params): Query<ProjectParam>, db_path: Arc<String>) -> Json<GraphData> {
    let project = params.project.as_deref().unwrap_or("default");
    let store = match Store::open(db_path.as_str()) { Ok(s) => s, Err(_) => return Json(GraphData { nodes: vec![], links: vec![] }) };
    let labels = [
        crate::models::NodeLabel::Character, crate::models::NodeLabel::Location,
        crate::models::NodeLabel::File, crate::models::NodeLabel::Scene,
        crate::models::NodeLabel::Item, crate::models::NodeLabel::Note,
    ];
    let mut nodes = Vec::new();
    for label in labels {
        if let Ok(ns) = store.find_nodes_by_label(project, label) {
            for n in ns {
                let group = match n.label {
                    crate::models::NodeLabel::Character => "character",
                    crate::models::NodeLabel::Location => "location",
                    crate::models::NodeLabel::File => "file",
                    crate::models::NodeLabel::Scene => "scene",
                    crate::models::NodeLabel::Item => "item",
                    _ => "note",
                };
                nodes.push(GraphNode { id: n.id, name: n.name, label: n.label.as_str().to_string(), group: group.to_string(), props: n.properties.inner });
            }
        }
    }
    let mut links = Vec::new();
    if let Ok(edges) = store.find_all_edges(project) {
        for e in edges { links.push(GraphLink { source: e.source_id, target: e.target_id, edge_type: e.edge_type.as_str().to_string() }); }
    }
    Json(GraphData { nodes, links })
}

async fn projects_handler(db_path: Arc<String>) -> Json<Vec<String>> {
    let store = match Store::open(db_path.as_str()) { Ok(s) => s, Err(_) => return Json(vec![]) };
    Json(store.list_projects().unwrap_or_default())
}

async fn index_html() -> Html<&'static str> { Html(HTML_PAGE) }

const HTML_PAGE: &str = r###"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>novelbase 知识图谱</title>
<script src="https://d3js.org/d3.v7.min.js"></script>
<style>
:root {
  --bg-deep: #06090f;
  --bg-surface: #0a161a;
  --bg-card: #0e2028;
  --bg-sidebar: #0c1a20;
  --fg: #e0eded;
  --fg-dim: #8aa8a8;
  --fg-faint: #4a6a6a;
  --accent: #1DA27E;
  --accent-dim: #1C8585;
  --border: rgba(26,58,64,0.3);
  --glow: rgba(29,162,126,0.15);
  --radius: 8px;
}
* { margin:0; padding:0; box-sizing:border-box; }
body {
  font-family: "Inter", system-ui, -apple-system, "PingFang SC", "Microsoft YaHei", sans-serif;
  background:var(--bg-deep); color:var(--fg); overflow:hidden; font-size:13px;
}

/* Header */
#header {
  position:fixed; top:0; left:0; right:0; z-index:100; height:48px;
  background:var(--bg-surface); backdrop-filter:blur(8px);
  border-bottom:1px solid var(--border);
  display:flex; align-items:center; gap:16px; padding:0 20px;
}
#header .logo { width:10px; height:10px; border-radius:50%; background:var(--accent); box-shadow:0 0 8px var(--glow); flex-shrink:0; }
#header h1 { font-size:14px; font-weight:600; color:var(--fg); letter-spacing:0.3px; }
#header .sep { width:1px; height:24px; background:var(--border); flex-shrink:0; }
#header select {
  background:var(--bg-card); color:var(--fg); border:1px solid var(--border);
  border-radius:6px; padding:4px 10px; font-size:12px; outline:none; cursor:pointer;
}
#header select:focus { border-color:var(--accent); }
#header .stats { margin-left:auto; font-size:11px; color:var(--fg-dim); display:flex; gap:12px; }
#header .stats strong { color:var(--fg); font-weight:500; }

/* Sidebar */
#sidebar {
  position:fixed; top:48px; left:0; bottom:0; z-index:50; width:240px;
  background:var(--bg-sidebar); border-right:1px solid var(--border);
  display:flex; flex-direction:column; overflow:hidden;
}
#sidebar .search-box { padding:10px 12px; border-bottom:1px solid var(--border); }
#sidebar .search-box input {
  width:100%; padding:6px 10px; background:var(--bg-card); color:var(--fg);
  border:1px solid var(--border); border-radius:6px; font-size:12px; outline:none;
}
#sidebar .search-box input:focus { border-color:var(--accent); }
#sidebar .search-box input::placeholder { color:var(--fg-faint); }
#sidebar .node-list { flex:1; overflow-y:auto; padding:4px 0; }
#sidebar .node-list .nitem {
  display:flex; align-items:center; gap:8px; padding:4px 12px; cursor:pointer;
  transition:background 0.15s;
}
#sidebar .node-list .nitem:hover { background:rgba(29,162,126,0.08); }
#sidebar .node-list .nitem.hl { background:rgba(29,162,126,0.15); }
#sidebar .node-list .dot { width:7px; height:7px; border-radius:50%; flex-shrink:0; }
#sidebar .node-list .nname { font-size:12px; color:var(--fg); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
#sidebar .node-list .nlabel { font-size:10px; color:var(--fg-faint); margin-left:auto; white-space:nowrap; flex-shrink:0; }

/* Detail */
#detail {
  position:fixed; top:48px; right:0; bottom:0; z-index:50; width:300px;
  background:var(--bg-sidebar); border-left:1px solid var(--border);
  transform:translateX(100%); transition:transform 0.25s cubic-bezier(0.4,0,0.2,1); overflow-y:auto;
}
#detail.open { transform:translateX(0); }
#detail .dhead { padding:14px 16px; border-bottom:1px solid var(--border); display:flex; align-items:center; gap:10px; }
#detail .dhead .dot { width:10px; height:10px; border-radius:50%; flex-shrink:0; }
#detail .dhead .dname { font-size:14px; font-weight:600; flex:1; }
#detail .dhead .dclose { cursor:pointer; color:var(--fg-faint); font-size:18px; line-height:1; padding:0 4px; }
#detail .dhead .dclose:hover { color:var(--fg); }
#detail .dbadge { display:inline-block; padding:2px 8px; border-radius:4px; font-size:10px; letter-spacing:0.5px; }
#detail .dsection { padding:10px 16px; border-bottom:1px solid var(--border); }
#detail .dsection h3 { font-size:10px; font-weight:500; color:var(--fg-dim); text-transform:uppercase; letter-spacing:0.5px; margin-bottom:6px; }
#detail .dprop { display:flex; padding:2px 0; font-size:12px; }
#detail .dprop .dk { color:var(--fg-dim); width:70px; flex-shrink:0; }
#detail .dprop .dv { color:var(--fg); word-break:break-all; }
#detail .dconn {
  display:flex; align-items:center; gap:6px; padding:4px 8px; border-radius:4px;
  cursor:pointer; font-size:12px; transition:background 0.15s;
}
#detail .dconn:hover { background:rgba(29,162,126,0.08); }
#detail .dconn .dot { width:5px; height:5px; border-radius:50%; flex-shrink:0; }
#detail .dconn .etype { font-size:9px; color:var(--fg-faint); text-transform:uppercase; font-family:monospace; margin-left:auto; }

/* Legend */
#legend {
  position:absolute; bottom:16px; left:256px; z-index:20;
  background:rgba(10,22,26,0.9); backdrop-filter:blur(8px);
  border:1px solid var(--border); border-radius:var(--radius);
  padding:8px 12px; font-size:11px; display:flex; flex-direction:column; gap:2px;
  pointer-events:none;
}
#legend .item { display:flex; align-items:center; gap:8px; }
#legend .dot { width:7px; height:7px; border-radius:50%; }
#legend .lname { color:var(--fg-dim); }

/* Tooltip */
#tooltip {
  position:fixed; z-index:200; pointer-events:none;
  background:rgba(14,32,40,0.95); backdrop-filter:blur(8px);
  border:1px solid var(--border); border-radius:var(--radius);
  padding:6px 10px; font-size:12px; display:none; max-width:220px;
}
#tooltip .tt-row { display:flex; align-items:center; gap:6px; }
#tooltip .dot { width:6px; height:6px; border-radius:50%; flex-shrink:0; }
#tooltip .tt-name { font-weight:500; }
#tooltip .tt-label { font-size:10px; color:var(--fg-faint); }

/* Graph */
#graph-area { position:fixed; top:48px; left:240px; right:0; bottom:0; }
svg.graph { width:100%; height:100%; display:block; }

/* Scroll */
::-webkit-scrollbar { width:4px; }
::-webkit-scrollbar-track { background:transparent; }
::-webkit-scrollbar-thumb { background:var(--border); border-radius:2px; }
</style>
</head>
<body>

<div id="header">
  <div class="logo"></div>
  <h1>novelbase</h1>
  <div class="sep"></div>
  <select id="project-select" onchange="loadGraph()"><option>加载中...</option></select>
  <div class="stats">
    <span>节点 <strong id="stat-nodes">0</strong></span>
    <span>边 <strong id="stat-edges">0</strong></span>
  </div>
</div>

<div id="sidebar">
  <div class="search-box"><input id="search-input" type="text" placeholder="搜索节点..." oninput="onSearch(this.value)"></div>
  <div class="node-list" id="node-list"></div>
</div>

<div id="detail">
  <div class="dhead">
    <div class="dot" id="detail-dot"></div>
    <div class="dname" id="detail-name"></div>
    <div class="dclose" onclick="closeDetail()">&times;</div>
  </div>
  <div style="padding:6px 16px"><span class="dbadge" id="detail-badge" style="background:rgba(29,162,126,0.15);color:var(--accent)"></span></div>
  <div id="detail-body"></div>
</div>

<div id="tooltip"></div>

<div id="graph-area">
  <svg class="graph" id="graph"></svg>
  <div id="legend">
    <div class="item"><span class="dot" style="background:#1DA27E"></span><span class="lname">角色</span></div>
    <div class="item"><span class="dot" style="background:#3b82f6"></span><span class="lname">地点</span></div>
    <div class="item"><span class="dot" style="background:#22c55e"></span><span class="lname">文件</span></div>
    <div class="item"><span class="dot" style="background:#f59e0b"></span><span class="lname">场景</span></div>
    <div class="item"><span class="dot" style="background:#a855f7"></span><span class="lname">笔记</span></div>
  </div>
</div>

<script>
const COLORS = { character:"#1DA27E", location:"#3b82f6", file:"#22c55e", scene:"#f59e0b", item:"#a855f7", note:"#64748b", other:"#4a6a6a" };
const EDGE_C = { KNOWS:"#1DA27E", LOCATED_IN:"#3b82f6", APPEARS_IN:"#22c55e", LEADS_TO:"#f59e0b", PART_OF:"#64748b", HAPPENS_AT:"#06b6d4", MENTIONS:"#a855f7", RELATED_TO:"#8aa8a8", FORESHADOWS:"#ec4899", TWIST:"#e05252", TAGGED_WITH:"#f97316" };

let graphData, svg, sim, g, link, node, label, selNode, detailOpen, allNodes = [];
const graphEl = document.getElementById("graph"), ga = document.getElementById("graph-area");
let W, H;
function resize() { W=ga.clientWidth; H=ga.clientHeight; graphEl.setAttribute("viewBox","0 0 "+W+" "+H); }
window.addEventListener("resize", resize);

svg = d3.select(graphEl); g = svg.append("g");
svg.call(d3.zoom().scaleExtent([0.05,10]).on("zoom", e => g.attr("transform", e.transform)));

const tip = document.getElementById("tooltip");
function showTip(d, ev) {
  tip.style.display="block";
  tip.innerHTML=`<div class="tt-row"><span class="dot" style="background:${COLORS[d.group]||COLORS.other}"></span><span class="tt-name">${esc(d.name)}</span></div><div class="tt-label">${esc(d.label)}</div>`;
  let x=ev.clientX+12, y=ev.clientY+12;
  const r=tip.getBoundingClientRect();
  if(x+r.width>window.innerWidth) x=ev.clientX-r.width-12;
  if(y+r.height>window.innerHeight) y=ev.clientY-r.height-12;
  tip.style.left=x+"px"; tip.style.top=y+"px";
}
function hideTip() { tip.style.display="none"; }

const detail=document.getElementById("detail");
function openDetail(d) {
  selNode=d; detailOpen=true; detail.classList.add("open");
  document.getElementById("detail-dot").style.background=COLORS[d.group]||COLORS.other;
  document.getElementById("detail-name").textContent=d.name;
  document.getElementById("detail-badge").textContent=d.label;
  let html="";
  if(d.props&&typeof d.props==="object") {
    const entries=Object.entries(d.props).filter(([k,v])=>v&&v!=="{}"&&v!=="");
    if(entries.length>0) {
      html+=`<div class="dsection"><h3>属性</h3>`;
      entries.forEach(([k,v])=>html+=`<div class="dprop"><span class="dk">${esc(k)}</span><span class="dv">${esc(String(v).slice(0,80))}</span></div>`);
      html+=`</div>`;
    }
  }
  const inEdges=graphData?graphData.links.filter(e=>e.target===d.id):[];
  const outEdges=graphData?graphData.links.filter(e=>e.source===d.id):[];
  const names={}; graphData&&graphData.nodes.forEach(n=>names[n.id]=n);
  if(outEdges.length>0) {
    html+=`<div class="dsection"><h3>发出 (${outEdges.length})</h3>`;
    outEdges.slice(0,20).forEach(e=>{const t=names[e.target];if(t)html+=`<div class="dconn" onclick="flyTo(${t.id})"><span class="dot" style="background:${COLORS[t.group]||COLORS.other}"></span>${esc(t.name)}<span class="etype">${e.edge_type}</span></div>`;});
    if(outEdges.length>20) html+=`<div style="font-size:11px;color:var(--fg-faint);padding:4px 8px">+${outEdges.length-20} 更多</div>`;
    html+=`</div>`;
  }
  if(inEdges.length>0) {
    html+=`<div class="dsection"><h3>被连接 (${inEdges.length})</h3>`;
    inEdges.slice(0,20).forEach(e=>{const s=names[e.source];if(s)html+=`<div class="dconn" onclick="flyTo(${s.id})"><span class="dot" style="background:${COLORS[s.group]||COLORS.other}"></span>${esc(s.name)}<span class="etype">${e.edge_type}</span></div>`;});
    if(inEdges.length>20) html+=`<div style="font-size:11px;color:var(--fg-faint);padding:4px 8px">+${inEdges.length-20} 更多</div>`;
    html+=`</div>`;
  }
  document.getElementById("detail-body").innerHTML=html||`<div class="dsection" style="color:var(--fg-faint)">无附加信息</div>`;
}
function closeDetail() { detail.classList.remove("open"); detailOpen=false; selNode=null; if(node) node.attr("opacity",1).attr("stroke",null); }
function flyTo(id) {
  if(!sim||!graphData) return;
  const n=graphData.nodes.find(x=>x.id===id); if(!n) return;
  if(node) { node.attr("opacity",d=>d.id===id?1:0.15).attr("stroke",d=>d.id===id?"#fff":null).attr("stroke-width",d=>d.id===id?2:null); }
  if(label) label.attr("opacity",d=>d.id===id?1:0.15);
  svg.transition().duration(750).call(d3.zoom().transform, d3.zoomIdentity.translate(-n.x*1+W/2,-n.y*1+H/2).scale(1));
  if(!detailOpen) openDetail(n);
  document.querySelectorAll(".nitem").forEach(el=>el.classList.toggle("hl",el.dataset.id==String(id)));
}

function onSearch(text) {
  if(!node) return;
  const q=text.toLowerCase();
  node.attr("opacity",d=>!q||d.name.toLowerCase().includes(q)||d.label.toLowerCase().includes(q)?1:0.06);
  if(label) label.attr("opacity",d=>!q||d.name.toLowerCase().includes(q)||d.label.toLowerCase().includes(q)?1:0.06);
  const list=document.getElementById("node-list");
  if(q) {
    const ms=allNodes.filter(n=>n.name.toLowerCase().includes(q)||n.label.toLowerCase().includes(q));
    list.innerHTML=ms.map(n=>`<div class="nitem" data-id="${n.id}" onclick="flyTo(${n.id})"><span class="dot" style="background:${COLORS[n.group]||COLORS.other}"></span><span class="nname">${esc(n.name)}</span><span class="nlabel">${esc(n.label)}</span></div>`).join("");
  } else {
    list.innerHTML=allNodes.map(n=>`<div class="nitem" data-id="${n.id}" onclick="flyTo(${n.id})"><span class="dot" style="background:${COLORS[n.group]||COLORS.other}"></span><span class="nname">${esc(n.name)}</span><span class="nlabel">${esc(n.label)}</span></div>`).join("");
  }
}
function esc(s) { const d=document.createElement("div"); d.textContent=s; return d.innerHTML; }

async function loadProjects() {
  try {
    const r=await fetch("/api/projects"); const ps=await r.json();
    const sel=document.getElementById("project-select");
    sel.innerHTML=ps.map(p=>`<option value="${esc(p)}">${esc(p)}</option>`).join("");
    if(ps.length>0) loadGraph();
  } catch(e) { console.error(e); }
}

async function loadGraph() {
  const project=document.getElementById("project-select").value; if(!project) return;
  try {
    const r=await fetch(`/api/graph?project=${encodeURIComponent(project)}`);
    graphData=await r.json(); renderGraph(graphData);
  } catch(e) { console.error(e); }
}

function renderGraph(data) {
  g.selectAll("*").remove(); if(!data.nodes.length) return;
  document.getElementById("stat-nodes").textContent=data.nodes.length;
  document.getElementById("stat-edges").textContent=data.links.length;

  const cc={}; data.nodes.forEach(n=>cc[n.id]=0);
  data.links.forEach(e=>{cc[e.source]=(cc[e.source]||0)+1; cc[e.target]=(cc[e.target]||0)+1;});
  const mx=Math.max(1,...Object.values(cc));
  data.nodes.forEach(n=>n.size=3+(cc[n.id]||0)/mx*14);

  allNodes=data.nodes;
  document.getElementById("node-list").innerHTML=data.nodes.map(n=>
    `<div class="nitem" data-id="${n.id}" onclick="flyTo(${n.id})"><span class="dot" style="background:${COLORS[n.group]||COLORS.other}"></span><span class="nname">${esc(n.name)}</span><span class="nlabel">${esc(n.label)}</span></div>`
  ).join("");

  W=ga.clientWidth; H=ga.clientHeight;
  sim=d3.forceSimulation(data.nodes)
    .force("link",d3.forceLink(data.links).id(d=>d.id).distance(70).strength(0.3))
    .force("charge",d3.forceManyBody().strength(-150))
    .force("center",d3.forceCenter(W/2,H/2))
    .force("collision",d3.forceCollide().radius(d=>d.size*1.5)).alphaDecay(0.02);

  svg.select("defs").remove();
  const defs=svg.append("defs");
  defs.selectAll("marker").data(Object.keys(EDGE_C)).join("marker")
    .attr("id",d=>"a-"+d).attr("viewBox","0 -4 8 8").attr("refX",d=>d==="KNOWS"?18:14).attr("refY",0)
    .attr("markerWidth",5).attr("markerHeight",5).attr("orient","auto")
    .append("path").attr("d","M0,-4L8,0L0,4").attr("fill",d=>EDGE_C[d]||"#555");

  link=g.append("g").selectAll("line").data(data.links).join("line")
    .attr("stroke",d=>EDGE_C[d.edge_type]||"#555").attr("stroke-width",0.7).attr("stroke-opacity",0.3)
    .attr("marker-end",d=>d.edge_type==="KNOWS"?"url(#a-KNOWS)":"");

  node=g.append("g").selectAll("circle").data(data.nodes).join("circle")
    .attr("r",d=>d.size).attr("fill",d=>COLORS[d.group]||COLORS.other)
    .attr("cursor","pointer")
    .on("mouseover",(e,d)=>showTip(d,e)).on("mouseout",hideTip)
    .on("click",(e,d)=>{e.stopPropagation();openDetail(d);})
    .call(d3.drag().on("start",(e,d)=>{if(!e.active)sim.alphaTarget(0.3).restart();d.fx=d.x;d.fy=d.y;})
      .on("drag",(e,d)=>{d.fx=e.x;d.fy=e.y;})
      .on("end",(e,d)=>{if(!e.active)sim.alphaTarget(0);d.fx=null;d.fy=null;}));

  label=g.append("g").selectAll("text").data(data.nodes).join("text")
    .text(d=>d.name).attr("font-size","10px").attr("dx",d=>d.size+3).attr("dy",3)
    .attr("fill","#8aa8a8").attr("pointer-events","none");

  sim.on("tick",()=>{
    link.attr("x1",d=>d.source.x).attr("y1",d=>d.source.y).attr("x2",d=>d.target.x).attr("y2",d=>d.target.y);
    node.attr("cx",d=>d.x).attr("cy",d=>d.y);
    label.attr("x",d=>d.x).attr("y",d=>d.y);
  });
  svg.on("click",()=>{if(detailOpen)closeDetail();});
}
resize();
loadProjects();
</script>
</body>
</html>"###;
