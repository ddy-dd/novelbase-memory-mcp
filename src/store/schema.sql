-- 项目表
CREATE TABLE IF NOT EXISTS projects (
                                        name TEXT PRIMARY KEY,
                                        root_path TEXT NOT NULL,
                                        indexed_at TEXT NOT NULL DEFAULT (datetime('now'))
    );

-- 节点表（角色、地点、场景……）
CREATE TABLE IF NOT EXISTS nodes (
                                     id INTEGER PRIMARY KEY AUTOINCREMENT,
                                     project TEXT NOT NULL REFERENCES projects(name) ON DELETE CASCADE,
    label TEXT NOT NULL,
    name TEXT NOT NULL,
    qualified_name TEXT NOT NULL UNIQUE,
    file_path TEXT,
    start_line INTEGER,
    end_line INTEGER,
    properties TEXT NOT NULL DEFAULT '{}'
    );

-- 边表（节点之间的关系）
CREATE TABLE IF NOT EXISTS edges (
                                     id INTEGER PRIMARY KEY AUTOINCREMENT,
                                     project TEXT NOT NULL REFERENCES projects(name) ON DELETE CASCADE,
    source_id INTEGER NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    target_id INTEGER NOT NULL REFERENCES nodes(id) ON DELETE CASCADE,
    type TEXT NOT NULL,
    properties TEXT NOT NULL DEFAULT '{}',
    UNIQUE(source_id, target_id, type)
    );

-- 文件哈希表（用于增量导入时跳过未修改的文件）
CREATE TABLE IF NOT EXISTS file_hashes (
                                              project TEXT NOT NULL,
                                              rel_path TEXT NOT NULL,
                                              sha256 TEXT NOT NULL,
    mtime_ns INTEGER NOT NULL,
    size INTEGER NOT NULL,
    PRIMARY KEY (project, rel_path)
    );


-- 索引
CREATE INDEX IF NOT EXISTS idx_nodes_project ON nodes(project);
CREATE INDEX IF NOT EXISTS idx_nodes_label ON nodes(label);
CREATE INDEX IF NOT EXISTS idx_nodes_qualified_name ON nodes(qualified_name);
CREATE INDEX IF NOT EXISTS idx_edges_source ON edges(source_id);
CREATE INDEX IF NOT EXISTS idx_edges_target ON edges(target_id);
CREATE INDEX IF NOT EXISTS idx_edges_type ON edges(type);