//! ParseChapterPass — 小说章节解析器（并行版）
//!
//! 能解析 8 种结构化章节：
//!
//! | 章节 | 作用 |
//! |------|------|
//! | `## 角色` | Character 节点 + AppearsIn 边 |
//! | `## 地点` | Location 节点 |
//! | `## 关系` | 角色之间的 Knows 边 |
//! | `## 情节脉络` | Scene 节点（开端/发展/高潮/结局） |
//! | `## 冲突` | Note 节点 |
//! | `## 环境` | Note 节点（时代/社会/风俗/文化） |
//! | `## 伏笔` | Foreshadows 边（叙事装置） |
//! | `## 反转` | Twist 节点（意外转折） |
//!
//! 角色行格式：`- 本名：别名1、别名2（身份，性格，外貌：xxx，动机：xxx，心理：xxx，结局：xxx）`

use crate::models::{Edge, EdgeType, Node, NodeLabel};
use crate::pipeline::{Context, PipelinePass, PipelineError};
use rayon::prelude::*;
use std::path::Path;

// ============================================================
// 解析结果
// ============================================================

/// 一个文件解析后的结果（用于跨线程传递）
#[derive(Debug)]
struct ParsedFile {
    file_id: i64,
    file_name: String,
    characters: Vec<CharacterInfo>,
    locations: Vec<LocationInfo>,
    relationships: Vec<RelationshipInfo>,
    plot_phases: Vec<PlotPhaseInfo>,
    conflicts: Vec<ConflictInfo>,
    environment: Option<SocialEnvInfo>,
    foreshadows: Vec<ForeshadowInfo>,
    twists: Vec<TwistInfo>,
}

/// 角色信息
#[derive(Debug)]
struct CharacterInfo {
    name: String,
    aliases: String,          // 别名
    identity: String,         // 身份
    personality: String,      // 性格
    appearance: String,       // 外貌
    motivation: String,       // 动机
    psychology: String,       // 心理活动
    fate: String,             // 结局
    traits: String,           // 原始描述（兼容旧格式）
}

/// 地点信息
#[derive(Debug)]
struct LocationInfo {
    name: String,
    description: String,
}

/// 关系信息
#[derive(Debug)]
struct RelationshipInfo {
    from: String,
    to: String,
    rel_type: String,
    description: String,
}

/// 情节阶段信息
#[derive(Debug)]
struct PlotPhaseInfo {
    phase: String,       // 开端/发展/高潮/结局
    chapters: String,    // 1-5回
    description: String,
}

/// 矛盾冲突
#[derive(Debug)]
struct ConflictInfo {
    parties: String,     // 宝玉 vs 父亲
    nature: String,      // 仕途与自由
}

/// 社会环境/时代背景
#[derive(Debug)]
struct SocialEnvInfo {
    era: String,         // 时代
    society: String,     // 社会背景
    customs: String,     // 风俗
    culture: String,     // 文化
}

/// 伏笔信息
#[derive(Debug)]
struct ForeshadowInfo {
    source: String,      // 伏笔所在（如 "判词"）
    target: String,      // 指向什么（如 "人物结局"）
    description: String, // 描述
}

/// 反转信息
#[derive(Debug)]
struct TwistInfo {
    expectation: String, // 预期
    reality: String,     // 实际发生
    description: String, // 描述
}

// ============================================================
// Pass 主体
// ============================================================

/// 章节解析 pass — 提取角色、地点、关系
pub struct ParseChapterPass;

impl PipelinePass for ParseChapterPass {
    fn name(&self) -> &'static str {
        "parse_chapter"
    }

    fn run(&self, ctx: &mut Context) -> Result<(), PipelineError> {
        // 第1步：从 GraphBuffer 取出所有 File 节点
        let file_infos: Vec<(i64, String, String)> = ctx.graph
            .find_by_label(NodeLabel::File)
            .iter()
            .filter_map(|n| {
                n.file_path.as_ref().map(|p| (n.id, n.name.clone(), p.clone()))
            })
            .collect();

        if file_infos.is_empty() {
            return Ok(());
        }

        let repo_path = ctx.repo_path.to_string();
        let project_name = ctx.project_name.to_string();

        // 🔥 并行阶段：读文件 + 解析
        let results: Vec<Result<ParsedFile, PipelineError>> = file_infos
            .par_iter()
            .map(|(file_id, file_name, rel_path)| {
                let full_path = Path::new(&repo_path).join(rel_path);
                let content = std::fs::read_to_string(&full_path).map_err(PipelineError::Io)?;
                Ok(ParsedFile {
                    file_id: *file_id,
                    file_name: file_name.clone(),
                    characters: Self::extract_chars(&content),
                    locations: Self::extract_locations(&content),
                    relationships: Self::extract_relationships(&content),
                    plot_phases: Self::extract_plot_phases(&content),
                    conflicts: Self::extract_conflicts(&content),
                    environment: Self::extract_environment(&content),
                    foreshadows: Self::extract_foreshadows(&content),
                    twists: Self::extract_twists(&content),
                })
            })
            .collect();

        // 顺序阶段：合并到 GraphBuffer
        let mut char_count = 0;
        let mut loc_count = 0;
        let mut rel_count = 0;

        for result in &results {
            let parsed = match result {
                Ok(p) => p,
                Err(e) => { eprintln!("  跳过: {}", e); continue; }
            };

            // ── 处理角色 ──
            for ch in &parsed.characters {
                let qn = format!("{}.{}", project_name, ch.name);
                let mut node = Node::new(&project_name, NodeLabel::Character, &ch.name, &qn);

                // 别名、身份、性格、结局写入 properties
                if !ch.aliases.is_empty() {
                    node.properties.insert("aliases", ch.aliases.as_str());
                }
                if !ch.identity.is_empty() {
                    node.properties.insert("identity", ch.identity.as_str());
                }
                if !ch.personality.is_empty() {
                    node.properties.insert("personality", ch.personality.as_str());
                }
                if !ch.appearance.is_empty() {
                    node.properties.insert("appearance", ch.appearance.as_str());
                }
                if !ch.motivation.is_empty() {
                    node.properties.insert("motivation", ch.motivation.as_str());
                }
                if !ch.psychology.is_empty() {
                    node.properties.insert("psychology", ch.psychology.as_str());
                }
                if !ch.fate.is_empty() {
                    node.properties.insert("fate", ch.fate.as_str());
                }
                if !ch.traits.is_empty() {
                    node.properties.insert("traits", ch.traits.as_str());
                }

                let char_id = ctx.graph.upsert_node(node);
                char_count += 1;

                let edge = Edge::new(&project_name, char_id, parsed.file_id, EdgeType::AppearsIn);
                ctx.graph.insert_edge(edge);
            }

            // ── 处理地点 ──
            for loc in &parsed.locations {
                let qn = format!("{}.{}", project_name, loc.name);
                let mut node = Node::new(&project_name, NodeLabel::Location, &loc.name, &qn);
                if !loc.description.is_empty() {
                    node.properties.insert("description", loc.description.as_str());
                }
                let loc_id = ctx.graph.upsert_node(node);
                loc_count += 1;

                let edge = Edge::new(&project_name, loc_id, parsed.file_id, EdgeType::AppearsIn);
                ctx.graph.insert_edge(edge);
            }

            // ── 处理关系 ──
            for rel in &parsed.relationships {
                let a_qn = format!("{}.{}", project_name, rel.from);
                let b_qn = format!("{}.{}", project_name, rel.to);

                // 从 GraphBuffer 找这两个角色
                let a_id = ctx.graph.find_by_qn(&a_qn).map(|n| n.id);
                let b_id = ctx.graph.find_by_qn(&b_qn).map(|n| n.id);

                if let (Some(a_id), Some(b_id)) = (a_id, b_id) {
                    let edge_type = match rel.rel_type.to_lowercase().as_str() {
                        "located_in" | "位于" => EdgeType::LocatedIn,
                        "appears_in" | "出现在" => EdgeType::AppearsIn,
                        "leads_to" | "导致" => EdgeType::LeadsTo,
                        "part_of" | "属于" => EdgeType::PartOf,
                        "happens_at" | "发生在" => EdgeType::HappensAt,
                        "mentions" | "提及" => EdgeType::Mentions,
                        _ => EdgeType::Knows,
                    };

                    let mut edge = Edge::new(&project_name, a_id, b_id, edge_type);
                    if !rel.description.is_empty() {
                        edge.properties.insert("description", rel.description.as_str());
                    }
                    ctx.graph.insert_edge(edge);
                    rel_count += 1;
                }
            }

            // ── 处理情节阶段 ──
            for pp in &parsed.plot_phases {
                let qn = format!("{}.{}", project_name, pp.phase);
                let mut node = Node::new(&project_name, NodeLabel::Scene, &pp.phase, &qn);
                node.properties.insert("plot_phase", pp.phase.as_str());
                node.properties.insert("chapters", pp.chapters.as_str());
                if !pp.description.is_empty() {
                    node.properties.insert("description", pp.description.as_str());
                }
                let note_id = ctx.graph.upsert_node(node);
                let edge = Edge::new(&project_name, note_id, parsed.file_id, EdgeType::PartOf);
                ctx.graph.insert_edge(edge);
            }

            // ── 处理矛盾冲突 ──
            for cf in &parsed.conflicts {
                let qn = format!("{}.冲突.{}", project_name, cf.parties);
                let mut node = Node::new(&project_name, NodeLabel::Note, &cf.parties, &qn);
                node.properties.insert("type", "conflict");
                node.properties.insert("nature", cf.nature.as_str());
                let note_id = ctx.graph.upsert_node(node);
                let edge = Edge::new(&project_name, note_id, parsed.file_id, EdgeType::Mentions);
                ctx.graph.insert_edge(edge);
            }

            // ── 处理社会/时代背景 ──
            if let Some(env) = &parsed.environment {
                let qn = format!("{}.环境设定", project_name);
                let mut node = Node::new(&project_name, NodeLabel::Note, "环境设定", &qn);
                if !env.era.is_empty() { node.properties.insert("era", env.era.as_str()); }
                if !env.society.is_empty() { node.properties.insert("society", env.society.as_str()); }
                if !env.customs.is_empty() { node.properties.insert("customs", env.customs.as_str()); }
                if !env.culture.is_empty() { node.properties.insert("culture", env.culture.as_str()); }
                let note_id = ctx.graph.upsert_node(node);
                let edge = Edge::new(&project_name, note_id, parsed.file_id, EdgeType::Mentions);
                ctx.graph.insert_edge(edge);
            }

            // ── 处理伏笔 ──
            for fw in &parsed.foreshadows {
                let src_qn = format!("{}.伏笔.{}", project_name, fw.source);
                let tgt_qn = format!("{}.伏笔.{}", project_name, fw.target);
                let mut src = Node::new(&project_name, NodeLabel::Note, &fw.source, &src_qn);
                src.properties.insert("type", "foreshadow");
                src.properties.insert("description", fw.description.as_str());
                let src_id = ctx.graph.upsert_node(src);
                let mut tgt = Node::new(&project_name, NodeLabel::Note, &fw.target, &tgt_qn);
                tgt.properties.insert("type", "foreshadow_target");
                let tgt_id = ctx.graph.upsert_node(tgt);
                let mut edge = Edge::new(&project_name, src_id, tgt_id, EdgeType::Foreshadows);
                edge.properties.insert("description", fw.description.as_str());
                ctx.graph.insert_edge(edge);
                println!("  伏笔: {} → {}", fw.source, fw.target);
            }

            // ── 处理反转 ──
            for tw in &parsed.twists {
                let qn = format!("{}.反转.{}", project_name, tw.expectation);
                let mut node = Node::new(&project_name, NodeLabel::Note, &tw.expectation, &qn);
                node.properties.insert("type", "twist");
                node.properties.insert("reality", tw.reality.as_str());
                if !tw.description.is_empty() {
                    node.properties.insert("description", tw.description.as_str());
                }
                ctx.graph.upsert_node(node);
                println!("  反转: {} → {}", tw.expectation, tw.reality);
            }
        }

        let fw_count: usize = results.iter()
            .map(|r| r.as_ref().map(|p| p.foreshadows.len()).unwrap_or(0)).sum();
        let tw_count: usize = results.iter()
            .map(|r| r.as_ref().map(|p| p.twists.len()).unwrap_or(0)).sum();

        println!(
            "  ParseChapter: {} 角色, {} 地点, {} 关系, {} 情节, {} 冲突, {} 伏笔, {} 反转（{} 文件）",
            char_count, loc_count, rel_count,
            results.iter().map(|r| r.as_ref().map(|p| p.plot_phases.len()).unwrap_or(0)).sum::<usize>(),
            results.iter().map(|r| r.as_ref().map(|p| p.conflicts.len()).unwrap_or(0)).sum::<usize>(),
            fw_count, tw_count, results.len()
        );
        Ok(())
    }
}

// ============================================================
// 解析器
// ============================================================

impl ParseChapterPass {
    /// 解析 `## 角色` 章节
    ///
    /// 格式：
    /// - `- 本名` → 只有名字
    /// - `- 本名：描述` → 名字+描述
    /// - `- 本名：别名1、别名2（身份，性格，结局：xxx）` → 完整格式
    fn extract_chars(content: &str) -> Vec<CharacterInfo> {
        let mut chars = Vec::new();
        let mut in_section = false;

        for line in content.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("## ") {
                in_section = rest.trim() == "角色" || rest.trim() == "人物";
                continue;
            }
            if !in_section || t.starts_with("## ") { in_section = false; continue; }

            if let Some(item) = t.strip_prefix("- ") {
                let item = item.trim();
                if item.is_empty() { continue; }

                let (name, raw_desc) = match item.split_once(':') {
                    Some((n, d)) => (n.trim().to_string(), d.trim().to_string()),
                    None => (item.to_string(), String::new()),
                };
                // 也支持中文冒号
                let (name, raw_desc) = if raw_desc.is_empty() {
                    match item.split_once('：') {
                        Some((n, d)) => (n.trim().to_string(), d.trim().to_string()),
                        None => (item.to_string(), String::new()),
                    }
                } else {
                    (name, raw_desc)
                };

                let desc = raw_desc;

                // 解析结构化信息（可选）
                // 新格式：- 林黛玉：颦儿、潇湘妃子（小姐，多愁善感，结局：泪尽而逝）
                // 旧格式：- 张三：剑客，性格勇敢（无括号，整个描述当 traits）
                let parsed = Self::parse_character_entry(&name, &desc);

                chars.push(parsed);
            }
        }
        chars
    }

    /// 解析 `## 地点` 或 `## 场所` 章节
    ///
    /// 格式：
    /// - `- 地点名` → 只有名字
    /// - `- 地点名：描述` → 名字+描述
    fn extract_locations(content: &str) -> Vec<LocationInfo> {
        let mut locs = Vec::new();
        let mut in_section = false;

        for line in content.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("## ") {
                in_section = rest.trim() == "地点" || rest.trim() == "场所";
                continue;
            }
            if !in_section || t.starts_with("## ") { in_section = false; continue; }

            if let Some(item) = t.strip_prefix("- ") {
                let item = item.trim();
                if item.is_empty() { continue; }

                if let Some((name, desc)) = item.split_once(':') {
                    locs.push(LocationInfo { name: name.trim().to_string(), description: desc.trim().to_string() });
                } else if let Some((name, desc)) = item.split_once('：') {
                    locs.push(LocationInfo { name: name.trim().to_string(), description: desc.trim().to_string() });
                } else {
                    locs.push(LocationInfo { name: item.to_string(), description: String::new() });
                }
            }
        }
        locs
    }

    /// 解析 `## 关系` 章节
    ///
    /// 格式：
    /// - `- 角色A → 角色B：描述` → 角色A KNOWS 角色B
    /// - `- 角色A -> 角色B` → 默认 KNOWS
    fn extract_relationships(content: &str) -> Vec<RelationshipInfo> {
        let mut rels = Vec::new();
        let mut in_section = false;

        for line in content.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("## ") {
                in_section = rest.trim() == "关系" || rest.trim() == "关系图谱";
                continue;
            }
            if !in_section || t.starts_with("## ") { in_section = false; continue; }

            if let Some(item) = t.strip_prefix("- ") {
                let item = item.trim();
                if item.is_empty() { continue; }

                // 找 → 或 -> 分隔
                // 💡 "→" 在 UTF-8 中占 3 字节，"->" 占 2 字节
                //    直接用 starts_with 来判断哪个箭头，避免字节计算错误
                let from;
                let after_arrow;
                if let Some(pos) = item.find("→") {
                    from = item[..pos].trim().to_string();
                    after_arrow = item[pos + "→".len()..].trim().to_string();
                } else if let Some(pos) = item.find("->") {
                    from = item[..pos].trim().to_string();
                    after_arrow = item[pos + "->".len()..].trim().to_string();
                } else {
                    continue;
                }

                let (to_name, desc) = if let Some((to, d)) = after_arrow.split_once(':') {
                    (to.trim().to_string(), d.trim().to_string())
                } else if let Some((to, d)) = after_arrow.split_once('：') {
                    (to.trim().to_string(), d.trim().to_string())
                } else {
                    (after_arrow.to_string(), String::new())
                };

                rels.push(RelationshipInfo {
                    from: from.to_string(),
                    to: to_name,
                    rel_type: "knows".to_string(),
                    description: desc,
                });
            }
        }
        rels
    }

    /// 解析角色条目，支持两种格式：
    ///
    /// 旧格式（无括号，整个描述当 traits）：
    ///   `- 张三：剑客，性格勇敢`
    ///
    /// 新格式（有括号，别名+结构化信息）：
    ///   `- 林黛玉：颦儿、潇湘妃子（小姐，多愁善感，结局：泪尽而逝）`
    fn parse_character_entry(name: &str, desc: &str) -> CharacterInfo {
        // 尝试解析新格式：有 '（' 和 '）'
        if let Some(paren_start) = desc.find('（') {
            let after_open = &desc[paren_start + '（'.len_utf8()..];
            if let Some(rel_end) = after_open.find('）') {
                let before_paren = desc[..paren_start].trim();
                let inside = after_open[..rel_end].trim();

                // 按 '，' 分割括号内的字段
                // 支持特殊字段前缀：外貌：动机：心理：结局：
                // 无前缀的按顺序：第一个→身份，第二个→性格
                let mut identity = String::new();
                let mut personality = String::new();
                let mut appearance = String::new();
                let mut motivation = String::new();
                let mut psychology = String::new();
                let mut fate = String::new();

                for segment in inside.split('，') {
                    let seg = segment.trim();
                    if seg.is_empty() { continue; }
                    // 识别带前缀的字段
                    let handled = if let Some(f) = seg.strip_prefix("外貌：").or_else(|| seg.strip_prefix("外貌:")) {
                        appearance = f.trim().to_string(); true
                    } else if let Some(f) = seg.strip_prefix("动机：").or_else(|| seg.strip_prefix("动机:")) {
                        motivation = f.trim().to_string(); true
                    } else if let Some(f) = seg.strip_prefix("心理：").or_else(|| seg.strip_prefix("心理:")) {
                        psychology = f.trim().to_string(); true
                    } else if let Some(f) = seg.strip_prefix("结局：").or_else(|| seg.strip_prefix("结局:")) {
                        fate = f.trim().to_string(); true
                    } else { false };

                    if !handled {
                        if identity.is_empty() { identity = seg.to_string(); }
                        else if personality.is_empty() { personality = seg.to_string(); }
                    }
                }

                return CharacterInfo {
                    name: name.to_string(),
                    aliases: before_paren.to_string(),
                    identity, personality,
                    appearance, motivation, psychology, fate,
                    traits: String::new(),
                };
            }
        }

        // 旧格式兜底：整个描述当 traits
        CharacterInfo {
            name: name.to_string(),
            aliases: String::new(),
            identity: String::new(),
            personality: String::new(),
            appearance: String::new(),
            motivation: String::new(),
            psychology: String::new(),
            fate: String::new(),
            traits: desc.to_string(),
        }
    }

    /// 解析 `## 情节脉络` 或 `## 结构` 章节
    ///
    /// 格式：
    /// - `- 开端：1-5回，描写`
    /// - `- 发展：6-80回，矛盾积累`
    fn extract_plot_phases(content: &str) -> Vec<PlotPhaseInfo> {
        let mut phases = Vec::new();
        let mut in_section = false;
        for line in content.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("## ") {
                in_section = rest.trim() == "情节脉络" || rest.trim() == "结构";
                continue;
            }
            if !in_section || t.starts_with("## ") { in_section = false; continue; }
            if let Some(item) = t.strip_prefix("- ") {
                let item = item.trim();
                if item.is_empty() { continue; }
                // 格式：阶段: 回目，描述
                if let Some((phase, rest)) = item.split_once('：').or_else(|| item.split_once(':')) {
                    let phase = phase.trim();
                    if let Some((chapters, desc)) = rest.split_once('，') {
                        phases.push(PlotPhaseInfo {
                            phase: phase.to_string(),
                            chapters: chapters.trim().to_string(),
                            description: desc.trim().to_string(),
                        });
                    } else {
                        phases.push(PlotPhaseInfo {
                            phase: phase.to_string(),
                            chapters: rest.trim().to_string(),
                            description: String::new(),
                        });
                    }
                }
            }
        }
        phases
    }

    /// 解析 `## 冲突` 章节
    ///
    /// 格式：
    /// - `- 宝玉 vs 父亲 → 仕途与自由的矛盾`
    fn extract_conflicts(content: &str) -> Vec<ConflictInfo> {
        let mut conflicts = Vec::new();
        let mut in_section = false;
        for line in content.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("## ") {
                in_section = rest.trim() == "冲突" || rest.trim() == "矛盾";
                continue;
            }
            if !in_section || t.starts_with("## ") { in_section = false; continue; }
            if let Some(item) = t.strip_prefix("- ") {
                let item = item.trim();
                if item.is_empty() { continue; }
                if let Some((parties, nature)) = item.split_once("→").or_else(|| item.split_once("->")) {
                    conflicts.push(ConflictInfo {
                        parties: parties.trim().to_string(),
                        nature: nature.trim().to_string(),
                    });
                } else if let Some((parties, nature)) = item.split_once('：') {
                    conflicts.push(ConflictInfo {
                        parties: parties.trim().to_string(),
                        nature: nature.trim().to_string(),
                    });
                }
            }
        }
        conflicts
    }

    /// 解析 `## 环境` 章节（单文件，每个字段一行）
    ///
    /// 格式：
    /// - `- 时代：清朝乾隆年间`
    /// - `- 社会：封建大家族`
    /// - `- 风俗：元宵节、诗社`
    /// - `- 文化：诗词、戏曲`
    fn extract_environment(content: &str) -> Option<SocialEnvInfo> {
        let mut env = SocialEnvInfo {
            era: String::new(), society: String::new(),
            customs: String::new(), culture: String::new(),
        };
        let mut in_section = false;
        let mut found = false;
        for line in content.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("## ") {
                in_section = rest.trim() == "环境" || rest.trim() == "背景";
                continue;
            }
            if !in_section || t.starts_with("## ") { in_section = false; continue; }
            if let Some(item) = t.strip_prefix("- ") {
                let item = item.trim();
                if let Some((k, v)) = item.split_once('：').or_else(|| item.split_once(':')) {
                    found = true;
                    match k.trim() {
                        "时代" => env.era = v.trim().to_string(),
                        "社会" => env.society = v.trim().to_string(),
                        "风俗" => env.customs = v.trim().to_string(),
                        "文化" => env.culture = v.trim().to_string(),
                        _ => {}
                    }
                }
            }
        }
        if found { Some(env) } else { None }
    }

    /// 解析 `## 伏笔` 章节
    ///
    /// 格式：
    /// - `- 判词 → 人物结局：金陵十二钗判词预示命运`
    fn extract_foreshadows(content: &str) -> Vec<ForeshadowInfo> {
        let mut result = Vec::new();
        let mut in_section = false;
        for line in content.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("## ") {
                in_section = rest.trim() == "伏笔" || rest.trim() == "铺垫";
                continue;
            }
            if !in_section || t.starts_with("## ") { in_section = false; continue; }
            if let Some(item) = t.strip_prefix("- ") {
                let item = item.trim();
                if item.is_empty() { continue; }
                // 格式：来源 → 目标：描述
                if let Some((arrow_part, desc)) = item.split_once('：').or_else(|| item.split_once(':')) {
                    if let Some((src, tgt)) = arrow_part.split_once("→").or_else(|| arrow_part.split_once("->")) {
                        result.push(ForeshadowInfo {
                            source: src.trim().to_string(),
                            target: tgt.trim().to_string(),
                            description: desc.trim().to_string(),
                        });
                    }
                }
            }
        }
        result
    }

    /// 解析 `## 反转` 章节
    ///
    /// 格式：
    /// - `- 预期：金玉良缘 → 实际：宝玉出家`
    fn extract_twists(content: &str) -> Vec<TwistInfo> {
        let mut result = Vec::new();
        let mut in_section = false;
        for line in content.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("## ") {
                in_section = rest.trim() == "反转" || rest.trim() == "转折";
                continue;
            }
            if !in_section || t.starts_with("## ") { in_section = false; continue; }
            if let Some(item) = t.strip_prefix("- ") {
                let item = item.trim();
                if item.is_empty() { continue; }
                // 格式：预期：xxx → 实际：yyy
                if let Some((before, after)) = item.split_once("→").or_else(|| item.split_once("->")) {
                    let expectation = before.trim()
                        .strip_prefix("预期：").or_else(|| before.trim().strip_prefix("预期:"))
                        .unwrap_or(before.trim())
                        .to_string();
                    let reality = after.trim()
                        .strip_prefix("实际：").or_else(|| after.trim().strip_prefix("实际:"))
                        .unwrap_or(after.trim())
                        .to_string();
                    result.push(TwistInfo {
                        expectation,
                        reality,
                        description: String::new(),
                    });
                }
            }
        }
        result
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── 角色提取 ──

    #[test]
    fn test_extract_chars_simple() {
        let content = "\
# 第一章
## 角色
- 张三
- 李四
";
        let chars = ParseChapterPass::extract_chars(content);
        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0].name, "张三");
        assert_eq!(chars[1].name, "李四");
    }

    #[test]
    fn test_extract_chars_with_traits() {
        let content = "\
## 角色
- 张三：剑客，性格勇敢
- 李四：书生
";
        let chars = ParseChapterPass::extract_chars(content);
        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0].name, "张三");
        assert_eq!(chars[0].traits, "剑客，性格勇敢");
    }

    #[test]
    fn test_extract_chars_with_alias() {
        // 新格式：别名在括号前，结构化信息在括号内
        let content = "\
## 角色
- 林黛玉：颦儿、潇湘妃子（小姐，多愁善感，结局：泪尽而逝）
";
        let chars = ParseChapterPass::extract_chars(content);
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].name, "林黛玉");
        assert_eq!(chars[0].aliases, "颦儿、潇湘妃子");
        assert_eq!(chars[0].identity, "小姐");
        assert_eq!(chars[0].personality, "多愁善感");
        assert_eq!(chars[0].fate, "泪尽而逝");
    }

    // ── 地点提取 ──

    #[test]
    fn test_extract_locations() {
        let content = "\
## 地点
- 大观园：贾府园林
- 潇湘馆
";
        let locs = ParseChapterPass::extract_locations(content);
        assert_eq!(locs.len(), 2);
        assert_eq!(locs[0].name, "大观园");
        assert_eq!(locs[0].description, "贾府园林");
        assert_eq!(locs[1].name, "潇湘馆");
    }

    #[test]
    fn test_extract_locations_stops_at_next_header() {
        let content = "\
## 地点
- 大观园
## 角色
- 张三
";
        let locs = ParseChapterPass::extract_locations(content);
        assert_eq!(locs.len(), 1);
    }

    // ── 关系提取 ──

    #[test]
    fn test_extract_relationships() {
        let content = "\
## 关系
- 贾宝玉 → 林黛玉：表兄妹
- 张三 -> 李四
";
        let rels = ParseChapterPass::extract_relationships(content);
        assert_eq!(rels.len(), 2);
        assert_eq!(rels[0].from, "贾宝玉");
        assert_eq!(rels[0].to, "林黛玉");
        assert_eq!(rels[0].description, "表兄妹");
        assert_eq!(rels[1].from, "张三");
        assert_eq!(rels[1].to, "李四");
    }

    // ── 增强角色字段 ──

    #[test]
    fn test_extract_char_with_full_fields() {
        // 完整格式：别名（身份，性格，外貌：xxx，动机：xxx，心理：xxx，结局：xxx）
        let content = "\
## 角色
- test：别名A、别名B（身份，性格，外貌：英俊，动机：求知，心理：平静，结局：善终）
";
        let chars = ParseChapterPass::extract_chars(content);
        assert_eq!(chars.len(), 1);
        assert_eq!(chars[0].name, "test");
        assert_eq!(chars[0].aliases, "别名A、别名B");
        assert_eq!(chars[0].identity, "身份");
        assert_eq!(chars[0].personality, "性格");
        assert_eq!(chars[0].appearance, "英俊");
        assert_eq!(chars[0].motivation, "求知");
        assert_eq!(chars[0].psychology, "平静");
        assert_eq!(chars[0].fate, "善终");
    }

    // ── 情节提取 ──

    #[test]
    fn test_extract_plot_phases() {
        let content = "\
## 情节脉络
- 开端：1-5回，人物出场
- 发展：6-80回，矛盾积累
- 高潮：81-100回，家族衰败
- 结局：101-120回，宝玉出家
";
        let phases = ParseChapterPass::extract_plot_phases(content);
        assert_eq!(phases.len(), 4);
        assert_eq!(phases[0].phase, "开端");
        assert_eq!(phases[0].chapters, "1-5回");
        assert_eq!(phases[0].description, "人物出场");
        assert_eq!(phases[3].phase, "结局");
    }

    // ── 冲突提取 ──

    #[test]
    fn test_extract_conflicts() {
        let content = "\
## 冲突
- 宝玉 vs 父亲 → 仕途与自由的矛盾
- 黛玉 vs 宝钗：爱情竞争
";
        let conflicts = ParseChapterPass::extract_conflicts(content);
        assert_eq!(conflicts.len(), 2);
        assert_eq!(conflicts[0].parties, "宝玉 vs 父亲");
        assert_eq!(conflicts[0].nature, "仕途与自由的矛盾");
        assert_eq!(conflicts[1].parties, "黛玉 vs 宝钗");
    }

    // ── 环境提取 ──

    #[test]
    fn test_extract_environment() {
        let content = "\
## 环境
- 时代：清朝
- 社会：封建家族
- 风俗：元宵节
- 文化：诗词
";
        let env = ParseChapterPass::extract_environment(content).unwrap();
        assert_eq!(env.era, "清朝");
        assert_eq!(env.society, "封建家族");
        assert_eq!(env.customs, "元宵节");
        assert_eq!(env.culture, "诗词");
    }
}
