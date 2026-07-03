# novelbase-memory-mcp

小说创作知识图谱 MCP 服务器。基于知识图谱 + 向量语义搜索，为原创或续写小说提供**结构化的人物管理、情节追踪、伏笔管理和语义搜索**能力。

> 受 [codebase-memory-mcp](https://github.com/DeusData/codebase-memory-mcp) 启发，将代码知识图谱的架构模式应用于小说创作领域。

![Web UI 截图](docs/ui-screenshot.png)

---

## 功能总览

### 📖 知识图谱
| 功能 | 说明 |
|------|------|
| **人物管理** | 角色名、别名、身份、性格、外貌、动机、心理、结局（8 维属性） |
| **地点管理** | 场景名称、描述 |
| **关系图谱** | 角色间的 Knows/AppearsIn/RelatedTo 等 11 种关系边 |
| **情节追踪** | 开端/发展/高潮/结局四阶段标记 |
| **矛盾冲突** | 对手方 + 矛盾本质的记录 |
| **环境设定** | 时代、社会、风俗、文化四维背景 |
| **伏笔追踪** | Foreshadows 边标记铺垫与呼应 |
| **反转记录** | Twist 节点记录意外转折 |
| **标签系统** | 按情节/文化/时空等分类打标签 |
| **原文溯源** | 每个节点记录出自第几回、第几行 |

### 🔍 搜索
| 搜索方式 | 引擎 | 说明 |
|---------|------|------|
| 关键词搜索 | FTS5 + unicode61 | 直接匹配节点名称/内容 |
| 语义搜索 | fastembed + int8 量化 | 按"意思"搜索，512 维向量余弦相似度 |
| 标签筛选 | SQL 过滤 | 按角色/地点/场景等类型过滤 |

### 🖥️ 交互方式
| 方式 | 说明 |
|------|------|
| **MCP 服务器** | stdin/stdout JSON-RPC 2.0，任何语言可集成 |
| **CLI 命令** | 8 个子命令，支持命令行操作 |
| **Web UI** | D3.js 力导向图可视化 |

---

## 快速开始

```bash
# 1. 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. 编译
git clone <仓库地址>
cd novelbase-memory-mcp
cargo build --release

# 3. 初始化项目并导入小说
cargo run --release -- init 红楼梦
cargo run --release -- cli import ./小说稿件/ --project 红楼梦 --source original

# 4. 启动 MCP 服务器
cargo run --release -- server

# 或者启动 Web UI
cargo run --release -- ui --port 8080
# 浏览器打开 http://localhost:8080/
```

---

## MCP 协议

MCP（Model Context Protocol）基于 stdin/stdout 的 JSON-RPC 2.0 协议。任何语言只要可以启动子进程并读写标准输入/输出即可集成。

### 工具参考

#### `add_character`

添加角色。

- **参数**：`project`（项目名）, `name`（角色名）, `traits`（可选，角色特征）
- **返回**：成功消息

```json
{"method":"tools/call","params":{"name":"add_character","arguments":{"project":"红楼梦","name":"贾宝玉","traits":"叛逆多情"}}}
```

#### `list_characters`

列出项目所有角色。

- **参数**：`project`（项目名）
- **返回**：角色列表

```json
{"method":"tools/call","params":{"name":"list_characters","arguments":{"project":"红楼梦"}}}
```

#### `add_relationship`

添加角色关系。

- **参数**：`project`, `character_a`, `character_b`, `relationship_type`
- **relationship_type 可选**：`knows`, `located_in`, `appears_in`, `leads_to`, `part_of`, `happens_at`, `mentions`, `related_to`
- **返回**：成功消息

```json
{"method":"tools/call","params":{"name":"add_relationship","arguments":{"project":"红楼梦","character_a":"贾宝玉","character_b":"林黛玉","relationship_type":"knows"}}}
```

#### `search_graph`

搜索节点。

- **参数**：`project`, `label`（过滤类型）
- **label 可选**：`character`, `location`, `scene`, `chapter`, `plotline`, `timeline`, `item`, `note`
- **返回**：匹配节点列表

```json
{"method":"tools/call","params":{"name":"search_graph","arguments":{"project":"红楼梦","label":"character"}}}
```

### 协议流程

1. 启动服务端：`novelbase-memory-mcp server`
2. 发送 `initialize` 请求获取协议版本：
   ```json
   {"jsonrpc":"2.0","id":1,"method":"initialize"}
   ```
3. 调用 `tools/list` 获取可用工具列表
4. 调用 `tools/call` 执行具体工具

### 集成示例

#### Python

```python
import subprocess, json

server = subprocess.Popen(
    ["novelbase-memory-mcp", "server"],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True
)

def call(tool, args):
    req = {"jsonrpc":"2.0","id":1,"method":"tools/call",
           "params":{"name":tool,"arguments":args}}
    server.stdin.write(json.dumps(req) + "\n")
    server.stdin.flush()
    return json.loads(server.stdout.readline())

print(call("list_characters", {"project":"红楼梦"}))
```

#### JavaScript / Node.js

```javascript
import { spawn } from 'child_process';
const srv = spawn('novelbase-memory-mcp', ['server']);

function call(tool, args) {
    const req = {"jsonrpc":"2.0","id":1,"method":"tools/call",
                 "params":{"name":tool,"arguments":args}};
    srv.stdin.write(JSON.stringify(req) + '\n');
    return new Promise(r => srv.stdout.once('data', d => r(JSON.parse(d))));
}

console.log(await call('list_characters', {project:'红楼梦'}));
```

#### TypeScript（MCP SDK）

```typescript
import { Client } from '@modelcontextprotocol/sdk/client/index.js';
import { StdioClientTransport } from '@modelcontextprotocol/sdk/client/stdio.js';

const transport = new StdioClientTransport({command:'novelbase-memory-mcp', args:['server']});
const client = new Client({name:'my-app', version:'1.0.0'});
await client.connect(transport);

const tools = await client.listTools();
const result = await client.callTool({name:'add_character',
    arguments:{project:'红楼梦', name:'贾宝玉', traits:'叛逆多情'}});
```

#### Rust

```rust
use std::process::{Command, Stdio};
use std::io::{BufRead, BufReader, Write};

let mut child = Command::new("novelbase-memory-mcp").arg("server")
    .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
let mut stdin = child.stdin.take().unwrap();
let mut stdout = BufReader::new(child.stdout.take().unwrap());

writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"tools/list"}}"#)?;
let mut response = String::new();
stdout.read_line(&mut response)?;
println!("{}", response);
```

#### Java

```java
Process process = new ProcessBuilder("novelbase-memory-mcp", "server").start();
try (BufferedWriter w = new BufferedWriter(
        new OutputStreamWriter(process.getOutputStream()));
     BufferedReader r = new BufferedReader(
        new InputStreamReader(process.getInputStream()))) {
    w.write("{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/list\"}\n");
    w.flush();
    System.out.println(r.readLine());
}
```

#### Go

```go
cmd := exec.Command("novelbase-memory-mcp", "server")
stdin, _ := cmd.StdinPipe()
stdout, _ := cmd.StdoutPipe()
cmd.Start()

request, _ := json.Marshal(map[string]any{
    "jsonrpc": "2.0", "id": 1, "method": "tools/list",
})
stdin.Write(append(request, '\n'))

reader := bufio.NewReader(stdout)
response, _ := reader.ReadString('\n')
fmt.Println(response)
```

---

## CLI 参考

| 命令 | 说明 |
|------|------|
| `server` | 启动 MCP 服务器 |
| `ui --port 8080` | 启动 Web UI |
| `init <项目名>` | 初始化新项目 |
| `cli add-character <角色名> --project <项目>` | 添加角色 |
| `cli add-relationship <A> <B> --type <类型> --project <项目>` | 添加关系 |
| `cli list-characters <项目>` | 列出角色 |
| `cli search <关键词> --project <项目>` | 搜索（FTS5 + 语义） |
| `cli import <路径> --project <项目> --source <标记>` | 导入小说文件 |
| `config` | 配置管理 |

### 小说导入

```bash
# 原著导入
novelbase cli import ./chapters/ --project 三国演义 --source original

# 续写导入（新文件单独目录）
novelbase cli import ./续写/ --project 三国演义 --source continuation
```

`--source` 标记用于区分原著和续写，可在数据库中按来源过滤查询。

---

## 小说文件格式

每个章节保存为一个 `.md` 文件，支持以下 8 种结构化章节。

### 角色

```markdown
## 角色
- 本名：别名1、别名2（身份，性格，外貌：xxx，动机：xxx，心理：xxx，结局：xxx）
- 贾宝玉：怡红公子、宝二爷（公子，叛逆多情，外貌：面若中秋之月，结局：出家为僧）
- 林黛玉：颦儿、潇湘妃子（小姐，多愁善感，外貌：闲静时如姣花照水，结局：泪尽而逝）
```

支持简洁格式（向后兼容）：

```markdown
## 角色
- 张三：剑客，性格勇敢
- 李四
```

### 地点 / 关系 / 情节脉络 / 冲突 / 伏笔 / 反转 / 标签 / 环境

```markdown
## 地点
- 大观园：贾府省亲所建

## 关系
- 贾宝玉 → 林黛玉：表兄妹，木石前盟

## 情节脉络
- 开端：1-5回，人物出场
- 发展：6-80回，矛盾积累
- 结局：101-120回，树倒猢狲散

## 冲突
- 贾宝玉 vs 贾政 → 仕途与自由的矛盾

## 伏笔
- 判词 → 人物结局：金陵十二钗判词预示命运
- 刘姥姥进大观园 → 贾府败落后救巧姐

## 反转
- 预期：金玉良缘 → 实际：宝玉出家

## 标签
- 情节：元妃省亲、抄检大观园、黛玉葬花
- 文化：中医药、诗词、饮食、建筑
- 时空：乾隆年间、大观园、潇湘馆

## 环境
- 时代：清朝乾隆年间
- 社会：封建大家族，等级森严
- 风俗：元宵节、中秋节、诗社
- 文化：诗词、戏曲、园林
```

---

## 数据模型

### 节点类型

| 标签 | 说明 | 属性 |
|------|------|------|
| `Character` | 角色 | name, aliases, identity, personality, appearance, motivation, psychology, fate, chapter, source_line, source |
| `Location` | 地点 | name, description, chapter, source_line |
| `File` | 章节文件 | name, file_path, chapter, source |
| `Scene` | 情节阶段 | plot_phase, chapters |
| `Note` | 备注（冲突/伏笔/标签等） | type, description |

### 边类型（11 种）

| 类型 | 含义 | 小说场景举例 |
|------|------|------------|
| `KNOWS` | 认识/关联 | 贾宝玉 → 林黛玉：表兄妹 |
| `LOCATED_IN` | 位于 | 潇湘馆 → 大观园 |
| `APPEARS_IN` | 出现在 | 贾宝玉 → 第一章 |
| `LEADS_TO` | 导致 | 抄检大观园 → 晴雯被逐 |
| `PART_OF` | 从属于 | 第一章 → 红楼梦 |
| `HAPPENS_AT` | 发生在 | 黛玉葬花 → 大观园 |
| `MENTIONS` | 提及 | 第一章 → 贾宝玉 |
| `RELATED_TO` | 关联 | 一般关系 |
| `FORESHADOWS` | 伏笔 | 判词 → 人物结局 |
| `TWIST` | 反转 | 金玉良缘 → 宝玉出家 |
| `TAGGED_WITH` | 标签标记 | 第一章 → 标签"元妃省亲" |

---

## 数据库结构

| 表 | 用途 |
|----|------|
| `projects` | 小说项目 |
| `nodes` | 所有节点（角色/地点/场景等） |
| `edges` | 节点间关系边 |
| `node_vectors` | 语义向量（int8 量化，512 维） |
| `token_vectors` | 关键词 enriched 向量 |
| `nodes_fts` | FTS5 全文搜索索引 |
| `file_hashes` | 文件哈希（增量导入） |

### 语义搜索

```sql
-- 搜索与"孤高自傲"语义最接近的角色
SELECT n.name, cbm_cosine_i8(v.vector, ?) AS score
FROM nodes n JOIN node_vectors v ON n.id = v.node_id
WHERE n.project = '红楼梦' AND n.label = 'Character'
ORDER BY score DESC LIMIT 10;
```

搜索流程：
1. 用户输入关键词 → fastembed 生成 f32 向量
2. int8 量化 → SQLite BLOB
3. `cbm_cosine_i8()` SQLite 自定义函数算余弦相似度
4. 多关键词时取 min 分（所有关键词都相关才高分）
5. OOV 关键词自动回退到稀疏随机投影

---

## 架构

```
src/
  main.rs              入口（CLI + MCP 服务器）
  cli/mod.rs           clap 命令行定义（8 子命令）
  models/              数据模型（Node, Edge, NodeLabel, EdgeType）
  store/               SQLite 存储（CRUD + FTS5 + 向量）
    node.rs            Node 数据库操作
    edge.rs            Edge 数据库操作
    file_hashes.rs     增量导入哈希
    vectors.rs         向量存储 + 余弦函数 + 搜索
  pipeline/            多 pass 导入流水线
    passes/
      discover.rs      文件发现（.md 扫描）
      parse_chapter.rs 章节解析（角色/地点/关系/情节/伏笔等）
      embed.rs         语义向量生成
    graph_buf/         内存图缓冲区
  mcp/                 MCP 服务器（JSON-RPC）
    protocol.rs        协议类型
    tools.rs           4 个 MCP 工具
  ui/                  Web UI（axum + D3.js）
```

导入流程：

```
.md 文件 → DiscoverPass（发现文件）
         → ParseChapterPass（解析角色/地点/关系/伏笔…）
         → EmbeddingPass（生成语义向量）
         → GraphBuffer → dump_to_store → SQLite
```

---

## 技术栈

| 组件 | 选型 |
|------|------|
| 语言 | Rust 2021 edition |
| 存储 | SQLite (bundled, FTS5, 余弦函数) |
| CLI | clap v4 |
| 并行 | rayon |
| 语义搜索 | fastembed (bge-small-zh-v1.5, 512维 int8) |
| Web | axum + tokio + D3.js |
| 哈希 | sha2 |

---

## 配置

| 环境变量 | 默认值 | 说明 |
|---------|--------|------|
| `RUST_LOG` | `info` | 日志级别：`debug`, `info`, `warn`, `error` |

---

## 许可

MIT
