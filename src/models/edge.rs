//! 边类型 — 节点之间的关系
//!
//! # 学习重点
//! - 和 node.rs 一样的模式（enum + impl）
//! - 结构体包含引用另一个结构体的 ID（`source_id`/`target_id`）
//! - `#[serde(rename = "type")]` — serde 的重命名功能

use serde::{Deserialize, Serialize};

// ============================================================
// 边类型
// ============================================================

/// 边的类型 — 描述两个节点之间的关系
///
/// 对应 C 版的 edge type（CALLS、IMPORTS 等），但改成小说场景的语义
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EdgeType {
    /// 角色A 认识 角色B
    Knows,
    /// 角色/物品 位于 某地点
    LocatedIn,
    /// 角色/地点 出现在 某场景/章节
    AppearsIn,
    /// 场景A 导致 场景B（因果关系）
    LeadsTo,
    /// 从属于（场景 → 故事线，章节 → 项目）
    PartOf,
    /// 事件发生在某个时间点
    HappensAt,
    /// 章节/笔记 提及 角色/物品
    Mentions,
    /// 一般的关联关系
    RelatedTo,
    /// A 为 B 埋下伏笔（叙事装置）
    Foreshadows,
    /// 反转/意外转折（A 与预期相反）
    Twist,
}

impl EdgeType {
    /// 从字符串解析
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "KNOWS" | "Knows" | "knows" => Some(Self::Knows),
            "LOCATED_IN" | "LocatedIn" | "located_in" => Some(Self::LocatedIn),
            "APPEARS_IN" | "AppearsIn" | "appears_in" => Some(Self::AppearsIn),
            "LEADS_TO" | "LeadsTo" | "leads_to" => Some(Self::LeadsTo),
            "PART_OF" | "PartOf" | "part_of" => Some(Self::PartOf),
            "HAPPENS_AT" | "HappensAt" | "happens_at" => Some(Self::HappensAt),
            "MENTIONS" | "Mentions" | "mentions" => Some(Self::Mentions),
            "RELATED_TO" | "RelatedTo" | "related_to" => Some(Self::RelatedTo),
            "FORESHADOWS" | "Foreshadows" | "foreshadows" => Some(Self::Foreshadows),
            "TWIST" | "Twist" | "twist" => Some(Self::Twist),
            _ => None,
        }
    }

    /// 转为大写标签（用于 SQLite 存储）
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Knows => "KNOWS",
            Self::LocatedIn => "LOCATED_IN",
            Self::AppearsIn => "APPEARS_IN",
            Self::LeadsTo => "LEADS_TO",
            Self::PartOf => "PART_OF",
            Self::HappensAt => "HAPPENS_AT",
            Self::Mentions => "MENTIONS",
            Self::RelatedTo => "RELATED_TO",
            Self::Foreshadows => "FORESHADOWS",
            Self::Twist => "TWIST",
        }
    }

    /// 中文描述
    pub fn cn_name(&self) -> &'static str {
        match self {
            Self::Knows => "认识",
            Self::LocatedIn => "位于",
            Self::AppearsIn => "出现在",
            Self::LeadsTo => "导致",
            Self::PartOf => "从属于",
            Self::HappensAt => "发生在",
            Self::Mentions => "提及",
            Self::RelatedTo => "关联",
            Self::Foreshadows => "伏笔",
            Self::Twist => "反转",
        }
    }
}

// ============================================================
// 属性集合（复用 node 的 Properties）
// ============================================================

use super::node::Properties;

// ============================================================
// 边
// ============================================================

/// 图中的边 — 连接两个节点
///
/// 💡 和 C 版的 cbm_edge_t 对应
/// 注意 `source_id` 和 `target_id` 指向 `nodes` 表中的 `id`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// 边 ID
    pub id: i64,
    /// 所属项目
    pub project: String,
    /// 源节点 ID
    pub source_id: i64,
    /// 目标节点 ID
    pub target_id: i64,
    /// 边类型
    #[serde(rename = "type")]
    pub edge_type: EdgeType,
    /// 属性
    pub properties: Properties,
}

impl Edge {
    /// 创建新边
    pub fn new(project: &str, source_id: i64, target_id: i64, edge_type: EdgeType) -> Self {
        Self {
            id: 0,
            project: project.to_string(),
            source_id,
            target_id,
            edge_type,
            properties: Properties::new(),
        }
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_type_roundtrip() {
        assert_eq!(EdgeType::from_str("KNOWS"), Some(EdgeType::Knows));
        assert_eq!(EdgeType::from_str("knows"), Some(EdgeType::Knows));
        assert_eq!(EdgeType::from_str("UNKNOWN"), None);
    }

    #[test]
    fn test_edge_type_as_str() {
        assert_eq!(EdgeType::Knows.as_str(), "KNOWS");
        assert_eq!(EdgeType::LeadsTo.as_str(), "LEADS_TO");
    }

    #[test]
    fn test_edge_type_cn_name() {
        assert_eq!(EdgeType::Knows.cn_name(), "认识");
        assert_eq!(EdgeType::LeadsTo.cn_name(), "导致");
    }

    #[test]
    fn test_edge_creation() {
        let edge = Edge::new("test", 1, 2, EdgeType::Knows);
        assert_eq!(edge.source_id, 1);
        assert_eq!(edge.target_id, 2);
        assert_eq!(edge.edge_type, EdgeType::Knows);
    }
}
