//! 节点类型 — 知识图谱中的实体
//!
//! # 学习重点
//! - `enum` 定义（Rust 的 enum 比 C 强大多了）
//! - `impl` 给类型加方法
//! - `&'static str` 静态字符串引用
//! - `serde::Serialize/Deserialize` 自动 JSON 序列化
//! - `serde_json::Value` 动态 JSON 值

use serde::{Deserialize, Serialize};

// ============================================================
// 节点标签
// ============================================================

/// 节点标签 — 区分不同类型的图节点
///
/// 💡 Rust 的 enum vs C 的 enum：
/// - C: `enum NodeLabel { CHARACTER, LOCATION, ... }` —— 就是个整数
/// - Rust: `enum NodeLabel { Character, Location, ... }` —— 每个变体是独立的值
///
/// 区别 1：Rust 的 enum 可以带方法（通过 impl）
/// 区别 2：Rust 编译器会检查 match 是否穷举了所有变体
/// 区别 3：每个变体可以附带不同的数据（后面会学到）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeLabel {
    /// 角色（主角、配角、反派等）
    Character,
    /// 地点（城市、建筑、国家等）
    Location,
    /// 场景（具体的事件发生场景）
    Scene,
    /// 章节
    Chapter,
    /// 故事线（主线、支线）
    Plotline,
    /// 时间线事件
    Timeline,
    /// 重要物品
    Item,
    /// 笔记/灵感
    Note,
    /// 项目根节点
    Project,
    /// 源文件（markdown 文件）
    File,
    ///组织
    Organization,
}

impl NodeLabel {
    /// 从字符串解析标签
    ///
    /// 💡 `Option<Self>` — 可能返回 None
    ///    C 版：找不到返回 -1 或 NULL
    ///    Rust：找不到返回 None，类型系统保证你不会把 None 当成有效值用
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "character" | "Character" => Some(Self::Character),
            "location" | "Location" => Some(Self::Location),
            "scene" | "Scene" => Some(Self::Scene),
            "chapter" | "Chapter" => Some(Self::Chapter),
            "plotline" | "Plotline" => Some(Self::Plotline),
            "timeline" | "Timeline" => Some(Self::Timeline),
            "item" | "Item" => Some(Self::Item),
            "note" | "Note" => Some(Self::Note),
            "project" | "Project" => Some(Self::Project),
            "file" | "File" => Some(Self::File),
            "organization" | "Organization" => Some(Self::Organization),
            _ => None, // 💡 通配符 _ 匹配所有剩余情况
        }
    }

    /// 转为字符串
    ///
    /// 💡 `&'static str` — 静态字符串引用
    ///    生命周期是 `'static`，意味着这个字符串在程序运行期间一直存在
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Character => "Character",
            Self::Location => "Location",
            Self::Scene => "Scene",
            Self::Chapter => "Chapter",
            Self::Plotline => "Plotline",
            Self::Timeline => "Timeline",
            Self::Item => "Item",
            Self::Note => "Note",
            Self::Project => "Project",
            Self::File => "File",
            Self::Organization => "Organization",
        }
    }

    /// 获取中文名称（方便 AI 理解）
    pub fn cn_name(&self) -> &'static str {
        match self {
            Self::Character => "角色",
            Self::Location => "地点",
            Self::Scene => "场景",
            Self::Chapter => "章节",
            Self::Plotline => "故事线",
            Self::Timeline => "时间线",
            Self::Item => "物品",
            Self::Note => "笔记",
            Self::Project => "项目",
            Self::File => "文件",
            Self::Organization => "组织"
        }
    }
}

// ============================================================
// 属性集合
// ============================================================

/// 节点的属性集合（对应 C 版 properties_json）
///
/// 💡 `serde_json::Value` 可以表示任何 JSON 值
///    相当于 C 版手动拼接的 `{"key":"value"}` 字符串
///    但 Rust 版本有类型安全——你不会拼出不合法的 JSON
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Properties {
    /// 底层 JSON 值
    pub inner: serde_json::Value,
}

impl Properties {
    /// 创建空属性
    pub fn new() -> Self {
        Self {
            inner: serde_json::Value::Object(Default::default()),
        }
    }

    /// 从 JSON 字符串解析
    ///
    /// 💡 `Result<Self, serde_json::Error>`
    ///    解析可能失败，所以返回 Result
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json).map(|inner| Self { inner })
    }

    /// 获取属性值
    ///
    /// 💡 `get()` 返回 `Option<&Value>`
    ///    可能不存在 key，所以是 Option
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.inner.get(key)
    }

    /// 获取字符串属性值
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.inner.get(key).and_then(|v| v.as_str())
    }

    /// 设置属性值
    ///
    /// 💡 `impl Into<serde_json::Value>` — 泛型参数
    ///    可以传 String、&str、i32、bool 等，自动转换成 Value
    ///
    /// 用法:
    /// ```ignore
    /// props.insert("age", 25);
    /// props.insert("name", "张三");
    /// props.insert("is_alive", true);
    /// ```
    pub fn insert(&mut self, key: &str, value: impl Into<serde_json::Value>) {
        // 💡 `as_object_mut()` 返回 `Option<&mut Map>`
        //    unwrap 是因为我们保证 inner 一定是 Object
        self.inner.as_object_mut().unwrap().insert(key.to_string(), value.into());
    }

    /// 转为 JSON 字符串
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.inner).unwrap_or_else(|_| "{}".to_string())
    }
}

// ============================================================
// 节点
// ============================================================

/// 知识图谱中的节点
///
/// 💡 和 C 版的 cbm_node_t 对应，但：
/// - 用 `Option<String>` 代替可空 `char*`（NULL → None）
/// - 用 `Properties` struct 代替原始 JSON 字符串
/// - 可以派生的 trait（Debug/Clone）通过 `#[derive]` 自动生成
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// 节点 ID（0 表示尚未分配）
    pub id: i64,
    /// 所属项目名
    pub project: String,
    /// 节点标签
    pub label: NodeLabel,
    /// 短名称
    pub name: String,
    /// 全限定名（"project.chapter.scene_name"）
    pub qualified_name: String,
    /// 源文件路径（可选，从文件导入时才有）
    pub file_path: Option<String>,
    /// 开始行号
    pub start_line: Option<i32>,
    /// 结束行号
    pub end_line: Option<i32>,
    /// 自定义属性
    pub properties: Properties,
}

impl Node {
    /// 创建一个新节点
    ///
    /// 💡 这是 Rust 的"构造器模式"——用函数代替直接访问字段
    ///    可以在这里做校验或默认值填充
    pub fn new(
        project: &str,
        label: NodeLabel,
        name: &str,
        qualified_name: &str,
    ) -> Self {
        Self {
            id: 0,
            project: project.to_string(),
            label,
            name: name.to_string(),
            qualified_name: qualified_name.to_string(),
            file_path: None,
            start_line: None,
            end_line: None,
            properties: Properties::new(),
        }
    }

    /// 设置文件路径（Builder 模式）
    ///
    /// 💡 返回 `Self` 可以链式调用：`Node::new(...).with_file("foo.md")`
    pub fn with_file(mut self, file_path: &str) -> Self {
        self.file_path = Some(file_path.to_string());
        self
    }

    /// 设置行号范围
    pub fn with_lines(mut self, start: i32, end: i32) -> Self {
        self.start_line = Some(start);
        self.end_line = Some(end);
        self
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_label_roundtrip() {
        // 💡 `assert_eq!` 需要 `PartialEq` trait
        assert_eq!(NodeLabel::from_str("Character"), Some(NodeLabel::Character));
        assert_eq!(NodeLabel::from_str("character"), Some(NodeLabel::Character));
        assert_eq!(NodeLabel::from_str("unknown"), None);
    }

    #[test]
    fn test_node_label_as_str() {
        assert_eq!(NodeLabel::Character.as_str(), "Character");
        assert_eq!(NodeLabel::Location.as_str(), "Location");
    }

    #[test]
    fn test_node_label_cn_name() {
        assert_eq!(NodeLabel::Character.cn_name(), "角色");
        assert_eq!(NodeLabel::Location.cn_name(), "地点");
    }

    #[test]
    fn test_properties() {
        let mut props = Properties::new();
        props.insert("age", 25);
        props.insert("name", "张三");

        assert_eq!(props.get_str("name"), Some("张三"));
        assert_eq!(props.get("age").and_then(|v| v.as_i64()), Some(25));
        assert_eq!(props.get_str("not_exists"), None);
    }

    #[test]
    fn test_properties_json_roundtrip() {
        let mut props = Properties::new();
        props.insert("trait", "勇敢");
        let json = props.to_json();
        let parsed = Properties::from_json(&json).unwrap();
        assert_eq!(parsed.get_str("trait"), Some("勇敢"));
    }

    #[test]
    fn test_node_builder() {
        let node = Node::new("test_project", NodeLabel::Character, "张三", "test_project.张三")
            .with_file("第一章.md")
            .with_lines(1, 50);

        assert_eq!(node.name, "张三");
        assert_eq!(node.file_path, Some("第一章.md".to_string()));
        assert_eq!(node.start_line, Some(1));
        assert_eq!(node.end_line, Some(50));
    }
}
