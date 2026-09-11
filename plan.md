# HTTP Client Pro — 实施计划

> 依据：[spec.md](./spec.md)（HTTP Request in Editor 格式规范）
> 架构参考：[t8y2/dbx](https://github.com/t8y2/dbx)（Rust + Tauri 双工作区、单核多端、双传输前端）

## 1. 项目定位

构建一个支持 HTTP Request in Editor 格式的 HTTP 客户端工具，能够：

- 解析 `.http` / `.rest` 文件（多个请求以 `###` 分隔）
- 执行请求并展示响应
- 支持环境变量 `{{var}}`、消息体文件引用 `< file`、响应处理脚本 `> {% ... %}`、响应引用 `<> file`
- 覆盖 spec 第 2~4 章：词法、文法、执行（编码、空白、multipart、环境变量、JS ES5.1 响应脚本）

工具形态对标 JetBrains HTTP Client / VS Code REST Client，但采用 dbx 的“单核多端”架构以同时覆盖桌面、Web/Docker、CLI、MCP 四种交付场景。

## 2. 架构参考（来自 dbx）

直接借鉴 dbx 的核心架构决策：

| dbx 架构决策 | 本项目对应 |
|---|---|
| Rust Cargo workspace + pnpm 双工作区 monorepo | 同样采用双工作区 |
| 一个 `dbx-core` 共享给 4 个二进制（Tauri/web/mcp/cli） | 一个 `http-core` 共享给 Tauri/web/mcp/cli |
| Tauri v2 + Vue 3 + Vite 前端 | 同样采用 |
| 前端双传输适配（Tauri IPC ↔ axum HTTP/SSE/WS） | 同样采用，`lib/backend/` 集中抽象 |
| Feature flag 控制编译产物体积 | 用于切换 JS 引擎、MCP、Web 模块 |
| `vendor/` + `[patch.crates-io]` 处理上游兼容补丁 | 预留，目前不需要 |
| Release profile 极致体积优化（`lto`、`strip`、`opt-level=s`） | 采用，目标单二进制 <15 MB |
| MCP server 独立 npm 分发 + 平台子包 | 采用，便于 AI agent 接入 |
| utoipa 生成 OpenAPI，保持 HTTP 契约与 Rust 同步 | web 端采用 |

与 dbx 的关键差异：dbx 的 core 是“数据库驱动 + AI + 安全”，我们的 core 是“HTTP 文法解析 + 请求执行 + JS 响应脚本引擎”。

## 3. 技术选型

### 3.1 Rust 后端

| 关注点 | 选型 | 说明 |
|---|---|---|
| 工作区 | Cargo workspace, resolver = "2" | 成员：`src-tauri`、`crates/http-core`、`crates/http-web`、`crates/http-mcp`、`crates/http-cli` |
| 异步运行时 | tokio（full） | 同 dbx |
| HTTP 客户端 | reqwest（rustls-tls） | 主执行器；支持 multipart、流式 body、代理 |
| URL 处理 | url 2.x + 自实现 percent-encoding | spec 4.1.2 要求非 ASCII 编码、已编码不二次编码 |
| JSON | serde / serde_json | 配置、环境变量、响应解析 |
| JS 引擎（响应脚本） | rquickjs（QuickJS 绑定） | spec 4.5 要求 ES5.1；rquickjs 嵌入体积小、无外部依赖，契合“小体积”目标。备选：boa（纯 Rust，体积偏大） |
| 模板 | minijinja | 环境变量替换预渲染（如果需要复杂逻辑） |
| 加密 | aes-gcm + arg2id / pbkdf2 | 配置加密（同 dbx configCrypto） |
| 日志 | tracing + tracing-subscriber | 同 dbx |
| MCP SDK | rmcp 2.x | 同 dbx，复用其 transport-io |
| Web 框架 | axum 0.8 + tower-http（cors/fs/compression） | 同 dbx-web |
| OpenAPI | utoipa 5.x | 同 dbx，契约同步 |
| Tauri | tauri 2.x + tauri-build | 同 dbx |
| 文件监听 | notify | 监听 `.http` 文件外部修改 |
| 快照测试 | insta | parser 文法用例快照 |

### 3.2 前端

| 关注点 | 选型 | 说明 |
|---|---|---|
| 框架 | Vue 3.5（Composition API + SFC） | 同 dbx |
| 语言 | TypeScript ~6.0 | 同 dbx |
| 构建 | Vite 8 | 同 dbx |
| 状态 | Pinia 3 | stores：environment、request、response、history、settings |
| UI | Tailwind CSS 4 + shadcn-vue（reka-ui） | 同 dbx |
| 编辑器 | CodeMirror 6 | 自实现 `.http` 语言包（lang-http-editor）：请求分隔符、注释、method 高亮、env 变量 `{{}}` 折叠 |
| 主题 | @uiw/codemirror-theme-* | 同 dbx，9 主题 |
| JSON 查看 | 自实现 + shiki 高亮 | 响应体美化 |
| 图表 | ECharts 6 + vue-echarts | 响应耗时直方图、历史趋势 |
| 虚拟滚动 | vue-virtual-scroller | 大响应体浏览 |
| i18n | vue-i18n 11 | 中/英 |
| 序列化 | @msgpack/msgpack、cbor-x | 同 dbx，可选 |
| 测试 | Vitest 4 + happy-dom | 同 dbx |
| Lint | oxlint + oxfmt + husky | 同 dbx |

### 3.3 Tauri 插件（v2）

`log`、`opener`、`dialog`、`fs`、`shell`、`updater`（minisign-verify 签名）、`single-instance`、`window-state`、`clipboard-manager`、`deep-link`（`httpc://` scheme，用于“在 HTTP Client Pro 中打开”）。文件关联：`.http`、`.rest`。

## 4. 项目结构

```
http_client_pro/
├── Cargo.toml                  # Rust workspace 根
├── package.json                # 根 JS 包（dev/build/lint/test 脚本）
├── pnpm-workspace.yaml         # packages/* 工作区
├── spec.md                      # 规范（已存在）
├── plan.md                      # 本文件
├── src-tauri/                   # Tauri 桌面壳（crate `http_client`）
├── crates/
│   ├── http-core/              # 共享核心：parser + executor + JS 引擎 + 环境
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── lexer/          # 第 2 章：base symbols、line terminators、whitespace、comments、separators
│   │   │   ├── parser/         # 第 3 章：requests-file、request、request-line、target、headers、body、multipart、response-handler、response-ref、env-variable
│   │   │   ├── model/         # AST / IR：RequestsFile、Request、RequestTarget、Headers、MessageBody、MultipartField、ResponseHandler、ResponseRef
│   │   │   ├── execute/       # 第 4 章：encoding、whitespace trimming、variable substitution、multipart、dispatch
│   │   │   ├── env/           # 环境变量解析与替换
│   │   │   ├── handler/      # 响应脚本引擎（rquickjs）：client.global.set/get、response.body/status/headers、assert
│   │   │   ├── config.rs      # 连接/环境配置（加密存储）
│   │   │   └── error.rs
│   │   └── tests/             # 文法用例 + 快照（insta）
│   ├── http-web/              # axum Docker/Web 后端（routes/、sse、auth、state）
│   ├── http-mcp/             # rmcp MCP server（server、session、backend、transport）
│   └── http-cli/            # 薄 CLI 二进制
├── apps/
│   └── desktop/              # Vue 3 SPA
│       └── src/
│           ├── components/   # editor/、request-pane/、response-pane/、env-switcher/、history/
│           ├── composables/
│           ├── stores/      # environment、request、response、history、settings
│           ├── lib/
│           │   ├── backend/ # 平台适配：tauri.ts / http.ts + per-domain adapters
│           │   ├── codemirror/ # lang-http-editor 语言包
│           │   └── configCrypto.ts
│           ├── i18n/
│           └── types/
└── packages/                # npm 分发
    ├── cli/                # @http-client-pro/cli
    ├── mcp-server/        # @http-client-pro/mcp-server
    └── cli-{platform}/、mcp-{platform}/   # 平台子包
```

## 5. 核心模块设计

### 5.1 Lexer（spec 第 2 章）

按 spec 2.1~2.5 实现 token 流：

- `input-character`、`alpha`、`digit`、`identifier`（支持 unicode）
- `new-line`（LF / CR / CRLF）、`new-line-with-indent`（用于多行 path/header/body 续行）
- `whitespace`（SP/HT/FF）、`optional-whitespace` / `required-whitespace`
- `line-comment`（`#` / `//`）
- `request-separator`（`###` 可带尾部注释）

关键点：spec 允许 unicode 出现在 path / query / authority / fragment，lexer 需以 char 级而非字节级处理。

### 5.2 Parser（spec 第 3 章）

递归下降，产物为 `RequestsFile { requests: Vec<Request> }`。文法节点：

- `requests-file` / `request-with-separator` / `request`
- `request-line`：`[method ws] target [ws http-version]`，method 缺省 `GET`
- `request-target`：`origin-form` / `absolute-form` / `asterisk-form`
  - `absolute-form`：`[scheme '://'] hier-part ['?' query] ['#' fragment]`
  - `authority`：`host [':' port]`，host 支持 IPv6（`[::1]`）/IPv4/域名（unicode）
  - `absolute-path`：支持 `new-line-with-indent` 续行（spec 3.2.1.3）
  - `query` / `fragment`：支持多行续行
- `headers`：`field-name ':' ows field-value ows`，`field-value` 支持续行
- `message-body`：
  - `messages`：行级，遇 `<`、`<> `、`###` 终止
  - `input-file-ref`：`'<' ws file-path`
  - `multipart-form-data`：boundary 切分，每个 `multipart-field` 含可选 headers + messages
- `response-handler`：`'>' ws '{%' script '%}'` 或 `'>' ws file-path`
- `response-ref`：`'<>' ws file-path`
- `env-variable`：`'{{' ows identifier ows '}}'`

错误恢复：遇错时返回带位置的诊断（类似 dbx `sql_diagnostics`），不中断整个文件解析。

### 5.3 Executor（spec 第 4 章）

执行管线 `Request (AST) → Preprocess → Dispatch → Response`：

1. **变量替换**：展开 `{{var}}`（来自当前 environment，spec 4.4）
2. **编码**（spec 4.1）：
   - path / query：非 ASCII percent-encode；已编码（`%xx`）不二次编码
   - host：按 RFC3986（spec 4.1.1 标记为 TODO，先按 IDNA/punycode 处理）
   - body：按 `Content-Type` 决定编码，缺省 UTF-8
3. **空白修剪**（spec 4.2）：path/query 每行首尾 trim；in-place body 整体首尾 trim；文件引用 body 不 trim
4. **Multipart**（spec 4.3）：按 `Content-Disposition` 的 `filename` 判定 string vs file
5. **Dispatch**：reqwest 发送，捕获 status / headers / body / 耗时
6. **Response handler**（spec 4.5）：在 rquickjs 沙箱中执行，暴露 `client.global.set/get`、`response.{body,status,headers}`、`assert` 等 API；in-place 与 file 两种来源
7. **Response ref**：将响应按 `<> path` 持久化，便于历史对比

### 5.4 环境变量（spec 4.4 — TODO）

spec 仅给出 `{{var}}` 文法，未定义 environment 来源与替换语义。本项目约定：

- environment = 命名变量集，前端可在环境间切换（dev/staging/prod）
- 来源优先级：请求级 > 环境级 > 全局级
- 替换发生在 path / header / body（spec 3.2.6 已限定范围）
- 未定义变量：保留原文 + 诊断警告（不阻断执行）
- 值插入不做自动编码；如需编码由用户在变量值中写入 `%xx`

### 5.5 Response Handler API（spec 4.5 — TODO）

spec 仅要求 ES5.1。本项目在 rquickjs 中暴露的最小 API：

```js
// 全局
client.global.set("key", value);
client.global.get("key");
client.log(...args);
// response 对象
response.status;            // number
response.body;               // string | object（按 Content-Type 解析）
response.headers.value("Content-Type");   // string | undefined
response.headers.valueOf(name);
// 断言
assert(status === 200, "auth failed");
```

后续可扩展 `client.assert` 风格 API。

## 6. 前端设计

- **编辑器**：CodeMirror 6 + 自实现 `@http-client-pro/lang-http-editor`，提供：`###` 分隔符折叠、请求块卡片化、`{{env}}` 悬浮提示、行内执行按钮（请求行旁“Run” affordance）、`Cmd/Ctrl+Enter` 执行当前请求。
- **请求面板**：从 AST 渲染可编辑表单（method/target/headers/body tabs），编辑回写至 `.http` 文本（双向同步，类似 dbx 的“表结构编辑器 → DDL 预览”）。
- **响应面板**：status chip、耗时、headers 表、body（JSON 树 / 原始 / 预览）、历史对比（`<> ref` 命中时展示 diff）。
- **环境切换器**：顶栏下拉，切换后立即重渲染当前文件变量高亮。
- **历史**：持久化执行历史（Pinia + 本地存储），支持重放。
- **平台适配**（同 dbx `lib/backend/`）：`tauri.ts`（桌面 `invoke`）与 `http.ts`（Web/Docker REST + SSE 流式响应）共享同一接口；`configCrypto.ts` 在桌面用 Tauri safeStorage、浏览器用 `@noble/ciphers`。

## 7. 多端部署

| 端 | 后端 | 前端 | 交付 |
|---|---|---|---|
| 桌面 | `src-tauri`（Tauri IPC） | 本地 Vue bundle | Tauri 安装包（NSIS/pkg/AppImage） |
| Web/Docker | `http-web`（axum REST + SSE + WS） | `tower-http` 静态托管 | Docker 镜像 |
| CLI | `http-cli`（薄壳） | — | `@http-client-pro/cli` npm 包 |
| MCP | `http-mcp`（rmcp） | — | `@http-client-pro/mcp-server`，供 Claude Code/Cursor/Windsurf 调用 |

MCP 权限分层（同 dbx）：`read_only`（仅解析展示）/ `safe_write`（可执行 GET/HEAD/OPTIONS）/ `high_risk_write`（全方法），在 Settings → MCP 配置允许的文件/集合。

## 8. 实施阶段

### Phase 0 — 脚手架（基线）
- 双工作区初始化：Cargo workspace + pnpm workspace
- `http-core` 空 crate、`src-tauri` 最小 Tauri v2 应用、`apps/desktop` 最小 Vue 3 + Vite
- release profile 体积优化配置
- CI（lint/test/build）

### Phase 1 — Parser（spec 第 2~3 章）
- lexer 完整 token 集
- 递归下降 parser 产出 AST
- insta 快照覆盖 spec 所有 `__Example:__`（含 multipart、续行、env 变量、response handler/ref）
- 错误诊断带位置

### Phase 2 — Executor（spec 第 4.1~4.3）
- 变量替换 + 编码（path/query/host/body）
- 空白修剪规则
- multipart dispatch
- reqwest 集成 + 流式响应捕获

### Phase 3 — Response Handler（spec 第 4.5）
- rquickjs 沙箱集成（feature `js-handler`）
- `client.global`、`response.*`、`assert` API
- 全局变量持久化（跨请求共享）
- 文件引用脚本 `> script.js`

### Phase 4 — 前端 MVP
- CodeMirror `lang-http-editor` 语言包
- 请求块高亮 + Run 按钮 + `Cmd+Enter`
- 响应面板（status/headers/body/耗时）
- 环境切换器 + 变量高亮
- 桌面端 Tauri IPC 打通

### Phase 5 — 多端
- `http-web`（axum REST + SSE 流式响应 + utoipa OpenAPI）
- 前端 `lib/backend/http.ts` 适配，双传输切换
- Docker 镜像
- `http-cli`（执行单个 `.http` 文件 / 单个请求）
- `http-mcp`（rmcp，权限分层）

### Phase 6 — 打磨
- 历史与重放、响应 diff（`<> ref`）
- 配置加密导入/导出
- 自动更新（Tauri updater + minisign）
- i18n 中/英
- 性能基准（大 `.http` 文件、大响应体）

### Phase 7 — JetBrains 官方 example 全兼容

目标：`example/` 下四个官方示例（`GET.http`、`POST.http`、`RequestWithLoop.http`、`RequestWithScripts.http`）中的**每一个请求都能在本工具中解析、执行并通过其内嵌断言**。

依据：[spec.md](./spec.md) + JetBrains 2026.1 *HTTP Client in product code editor* 官方文档。

#### 7.0 差距分析（audit 结论）

对 `http-core` 逐特性审计后的缺口清单：

| # | 特性 | 出处 | 现状 | 缺口位置 |
|---|---|---|---|---|
| G1 | `//` 行注释（`//TIP ...`） | GET.http:14,41,47 / POST.http:18 | lexer 只识别 `#`，`//` 被当正文 | `lexer.rs` |
| G2 | doc tags `@no-redirect` / `@no-cookie-jar` / `@no-auto-encoding` | GET.http:20,24,28 | 注释行被 parser 直接丢弃 | `parser.rs:52-54`、`model.rs` |
| G3 | 裸 `HTTP/2` 版本号 | GET.http:51 | `is_http_version` 要求 `HTTP/d.d` | `parser.rs:503-515` |
| G4 | 输出重定向 `>> file` / `>>! file` | GET.http:42,48 | `>>` 被 `parse_simple_body` 吞入正文 | `parser.rs:199+`、`model.rs` |
| G5 | 前置请求脚本 `< {% ... %}` | Loop:2-8 / Scripts:54-59,69-75,85-90,108-110 | 被误解析为 `MessageBody::FileRef{path:"{%...%}"}` | `parser.rs:189-196` |
| G6 | 多行响应处理器 `> {% ⏎ ... ⏎ %}` | Loop:20-26 / Scripts 全部 | `handler_from` 只支持单行（`rfind("%}")`） | `parser.rs:642-653` |
| G7 | 动态变量 `{{$random.uuid}}` `{{$timestamp}}` `{{$random.integer()}}` | GET.http:36 / POST.http:41-43 | `scan_env_var` 标识符字符集仅 `[A-Za-z0-9_-]`，`.` 不匹配 | `env.rs:73-80` |
| G8 | 内置路径变量 `{{$historyFolder}}` `{{$projectRoot}}` | GET.http:42,48 | 无 | `env.rs` |
| G9 | JSONPath 模板 `{{$.clients..id}}`（递归下降） | Loop:14-17,33-34 | 无 JSONPath 求值器 | 新模块 |
| G10 | 多值模板 → 请求循环执行 N 次 | Loop 全文件 | `run_request` 只发一次 | `execute.rs`、`http-cli` |
| G11 | 变量值可为 JSON（数组/对象），非仅字符串 | Loop:3-7,21 | `Environment` = `HashMap<String,String>` | `env.rs` |
| G12 | x-www-form-urlencoded 正文格式化 + `%+`/`%&`/`%=` 转义 | POST.http:14-16 | `build_body` 无 urlencoded 分支 | `execute.rs` |
| G13 | 按请求差异化 client：重定向策略 / cookie jar / timeout / HTTP 版本 | GET.http:20,24,51 | `Dispatcher` 单一全局 client（`Policy::none()` + 30s） | `dispatch.rs` |
| G14 | `client.test(name, fn)` / `client.assert(cond,msg)` / `client.global.clearAll()` / `client.global.headers.set` | Scripts 全部 / Loop:22,24 | handler 只有顶层 `assert`、`client.global.set/get`、`client.log` | `handler.rs` |
| G15 | `response.contentType.mimeType/charset`、`response.headers.valueOf/valuesOf`、`response.cookies()` | Scripts:28,47 | 只有 `response.headers.value` | `handler.rs` |
| G16 | `request.*` 对象（method/url/body/headers/environment/variables/iteration/templateValue） | Loop:21,38,41 / Scripts:56,71,72,89 | 无 | `handler.rs` |
| G17 | `crypto.sha256()` / `crypto.hmac.sha256()` 链式 API | Scripts:55-57,70-73 | 无 | `handler.rs` |
| G18 | 全局函数 `jsonPath(obj, expr)` | Loop:23,45 | 无 | `handler.rs` |
| G19 | ES 模块 `import {x} from "./my-utils"` | Scripts:87,100 | rquickjs `loader` feature 已开但未接线 | `handler.rs` |
| G20 | 脚本内 `{{var}}` 替换、模板字符串、ES6 语法（`let`/`const`/箭头/反引号） | Loop / Scripts 全部 | 未验证 | `handler.rs` |
| G21 | 环境文件 `http-client.env.json` + `http-client.private.env.json`（后者优先） | Scripts:71 | CLI 只读公开 env，无私有 env、无 core 侧加载器 | `env.rs`、`http-cli` |
| G22 | 变量作用域优先级 environment > global > file(`@x = y`) > request | 官方文档 | 只有单层 `Environment` | `env.rs` |
| G23 | 批量执行整个文件 + 测试汇总 + 退出码 | 全部 example | `run_request` 单请求、不跑 handler | `http-cli/lib.rs` |
| G24 | JSON 无引号位置的字符串型动态变量自动加引号 | POST.http:41 | 无 | `execute.rs` |

已确认可用（无需改动）：JSON body、multipart（含 `< file` part）、`<> response-ref`、多行缩进 query/path/header、`{{var}}` 基础替换、percent-encoding 不二次编码、rquickjs 沙箱与超时中断、httpmock 集成测试骨架。

#### 7.1 Model 扩展（`model.rs`）

```rust
pub struct Request {
    // …既有字段…
    pub pre_request_script: Option<ResponseHandler>, // 复用 Inline/FileRef 两态
    pub output_redirect: Option<OutputRedirect>,     // >> / >>!
    pub tags: Vec<DocTag>,                           // @no-redirect 等
}

pub struct OutputRedirect { pub path: String, pub force: bool }

pub enum DocTag {
    NoRedirect, NoCookieJar, NoAutoEncoding, NoLog,
    Timeout { millis: u64 }, ConnectionTimeout { millis: u64 },
    Name { value: String },
}
```

#### 7.2 Lexer / Parser

- **G1**：`lexer.rs` 增加 `//` 前缀 → `Line::Comment`（`###` 优先级不变）。
- **G2**：`parser.rs` 不再丢弃注释行；在请求块起始处收集紧邻的 `#`/`//` 注释，正则扫描 `@(no-redirect|no-cookie-jar|no-auto-encoding|no-log|timeout|connection-timeout|name)\b(\s+\d+|\s+[^ ]+)?` 产出 `DocTag`。
- **G3**：`is_http_version` 放宽为 `HTTP/` + digits（`HTTP/2` → 2.0）；`execute.rs` 的 `http_version` 映射增加 `"HTTP/2" | "HTTP/2.0"` → `reqwest::Version::HTTP_2`。
- **G4**：`parse_simple_body` 的正文终止符集合增加 `>>`（含 `>>!`）；新增 `parse_output_redirect()`，在 `parse_response_ref()` 之后调用。
- **G5**：请求解析入口先探测 `< {%`（前置脚本），与 `< file`（正文文件引用）区分——判据为 `{%` 前缀；前置脚本同样支持多行 `%}` 收尾与 `> script.js` 文件引用。
- **G6**：新增 `parse_braced_script()` 通用扫描器：从 `{%` 起逐行累积，直到某行以 `%}` 结束（支持同行 `> {% ... %}` 与跨行两种形态）。`response_handler` 与 `pre_request_script` 共用。

#### 7.3 动态变量与作用域（`env.rs` 重构）

```rust
pub enum VarValue { Str(String), Json(serde_json::Value) }

pub struct Scope {          // 分层，查询按优先级穿透
    pub environment: BTreeMap<String, VarValue>, // http-client[.private].env.json
    pub global:      BTreeMap<String, VarValue>, // client.global.set
    pub file:        BTreeMap<String, VarValue>, // in-place `@name = value`
    pub request:     BTreeMap<String, VarValue>, // 前置脚本 request.variables.set
}
```

- **G7**：`scan_env_var` 标识符字符集扩展为 `[A-Za-z0-9_.$\-\[\]()'",= ]`（覆盖 `$random.integer(1,100)` 与 `$.clients..id`），仍以 `}}` 收尾；未闭合报错不变。
- **G7/G8**：新增 `dynamic.rs`——`resolve_dynamic(name, ctx) -> Option<VarValue>`：
  - `$uuid` / `$random.uuid` → uuid v4
  - `$timestamp` → unix 秒；`$isoTimestamp` → RFC3339 UTC
  - `$randomInt` → `[0,1000)`；`$random.integer()` → `[0,1000)`；`$random.integer(from,to)` → `[from,to]`
  - `$historyFolder` → `<projectRoot>/.http-history`（自动 mkdir）；`$projectRoot` → 工作目录
  - 未命中 `$` 前缀 → 回落 `Scope` 查询
- **G11**：`VarValue::Json` 允许 `request.variables.set("clients", [...])` 存入数组；替换到文本时，字符串直出、数字/布尔直出、对象/数组序列化为紧凑 JSON。
- **G21**：`env.rs` 新增 `load_env_files(dir, env_name) -> Scope`：读 `http-client.env.json`（`{"env": {"k": "v"}}`，JetBrains 格式），再用 `http-client.private.env.json` 覆盖同名键（私有优先）。
- **G22**：`Scope::get()` 按 environment → global → file → request 顺序查询（与官方文档一致：environment 最高）。
- **G24**：替换时若 `Content-Type` 含 `json`（或 body 以 `{`/`[` 开头），且占位符紧邻字符不是 `"`，且 `VarValue` 为字符串型 → 插入 JSON 转义后的带引号字面量；数字/布尔型裸插。

#### 7.4 JSONPath 与循环执行（新增 `jsonpath.rs`）

- **G9**：自实现最小 JSONPath（不引第三方 crate，避免语义分歧）：
  - `$` 根、`.field` 子字段、`[n]` 索引、`..field` 递归下降（DFS 收集所有同名键）
  - 返回值 `Vec<&serde_json::Value>`；单值场景取第 0 个
- **G10**：`execute.rs` 新增 `expand_iterations(request, scope) -> Vec<Iteration>`：
  1. 扫描 path/query/headers/body 中所有 `{{$.…}}` 模板，按出现顺序编号 `template_index`
  2. 对每个模板求 JSONPath 得值列表；迭代次数 = 各列表长度的最大值（长度不足者取模循环，与 JetBrains 行为一致）
  3. 每个 `Iteration { index, bindings: Vec<(template_index, VarValue)> }`
  4. 无 JSONPath 模板 → 单个 `Iteration { index: 0, bindings: [] }`
- **G16**：`request.iteration()` → `index`；`request.templateValue(i)` → `bindings[i].1`（注入 handler 上下文）。

#### 7.5 x-www-form-urlencoded（`execute.rs`）

- **G12**：`build_body` 增加分支——当 `Content-Type` 为 `application/x-www-form-urlencoded` 且 body 为 `Inline` 时：
  1. 先做 `%` 转义还原：`%+`→`+`、`%&`→`&`、`%=`→`=`、`%%`→`%`
  2. 按 `&` 切分键值对（跨行续行已在 parser 阶段拼接为多行文本，此处按行尾 `&` 或换行合并）
  3. 每对按第一个 `=` 分割，key/value 各自 trim 后 percent-encode（`form_urlencoded` 规则：空格→`+`）
  4. 以 `&` 连接输出
  - 例：`id = 999 &⏎value = content &⏎fact = IntelliJ %+ HTTP Client %= <3` → `id=999&value=content&fact=IntelliJ+%2B+HTTP+Client+%3D+%3C3`

#### 7.6 Dispatch 扩展（`dispatch.rs`）

- **G13**：`Dispatcher::send_with(req, opts)`，`opts` 由 `Request::tags` + `http_version` 推导：
  - `@no-redirect` → `redirect(Policy::none())`；否则 `Policy::limited(10)`
  - `@no-cookie-jar` → 不挂 jar；否则 `cookie_store(true)` + 进程级共享 `Arc<CookieStoreMutex>`
  - `@timeout N` / `@connection-timeout N` → 覆盖默认 30s
  - `HTTP/2` → `http2_prior_knowledge()` 或 `version(reqwest::Version::HTTP_2)`（reqwest 需开 `http2` feature）
  - `@no-auto-encoding` → 跳过 `encode_path`/`encode_query`，原样发送
  - client 按 opts 指纹缓存复用，避免每请求重建 TLS 上下文
- **G4 落盘**：`write_output_redirect(resp, redirect, scope)`——解析 `{{$historyFolder}}` 后写文件；`force=false` 且文件已存在 → 追加 `-1`/`-2` 后缀；`force=true` → 直接覆盖。
- Cargo：workspace `reqwest` features 增加 `http2`、`cookies`。

#### 7.7 JS 引擎扩展（`handler.rs`）

- **G14**：
  - `client.test(name, fn)` → 执行 `fn`，捕获异常，产出 `TestResult{name, passed, message}`；不再向上抛
  - `client.assert(cond, msg?)` → `cond` falsy 时抛 `AssertionError(msg)`（被 `client.test` 捕获）
  - `client.global.clearAll()` / `client.global.clear(key)` / `client.global.headers.set(name, value)`
  - `client.exit()`（标记中断后续迭代）
- **G15**：`response.contentType.{mimeType,charset}`（解析 `Content-Type` 头）、`response.headers.valueOf(name)`（首个值）、`response.headers.valuesOf(name)`（数组）、`response.cookies()`
- **G16**：注入 `request` 对象：`method`、`url()`、`body.tryGetSubstituted()`/`body.raw()`、`headers.valueOf(name)`、`environment.get(k)`、`variables.get(k)/set(k,v)`、`iteration()`、`templateValue(i)`。前置脚本与响应处理器共用同一注入器（前置脚本 `response` 为 `undefined`）。
- **G17**：`crypto.sha256()/sha512()/sha1()/md5()` 与 `crypto.hmac.{sha256,sha512,sha1,md5}()`，链式 `.withTextSecret(s)` → `.updateWithText(s)` → `.digest()` → `.toHex()/.toBase64()`。Cargo 增加 `sha2`、`sha1`、`md-5`、`hmac`、`hex`、`base64`。
- **G18**：全局 `jsonPath(obj, expr)` → 复用 §7.4 的 `jsonpath.rs`；单值返回值本身，多值返回数组。
- **G19**：rquickjs `loader` 接线——`ModuleLoader` 以脚本所在目录为 base 解析相对路径，支持省略 `.js` 扩展名；`BuiltinResolver` 兜底。前置脚本与 handler 均以 **module** 方式求值，使顶层 `import` 合法。
- **G20**：脚本源码在进入 QuickJS 前先跑一次 `scope.substitute()`，使 `{{var}}` 在脚本体内可用；`client.test` 的结果收集进 `HandlerState`，随 `DispatchResponse` 一并回传。
- 测试汇总：`HandlerOutcome { logs, tests: Vec<TestResult>, globals_delta, diagnostics }`。

#### 7.8 Runner 与 CLI 接线

- **G23**：`http-core` 新增 `runner.rs`——`run_file(path, opts) -> FileReport`：
  - 顺序执行文件内每个 `Request`；每个请求先跑前置脚本 → 展开迭代 → 逐迭代 dispatch → 跑响应处理器 → 写 `>>` / `<>`
  - `client.global` 跨请求持久（单个 `HandlerRuntime` 复用）
  - `FileReport { requests: Vec<RequestReport>, tests_passed, tests_failed, logs }`
- `http-cli`：`run file.http` 默认跑**全文件**（保留 `--request` 过滤）；新增 `--all`、输出测试汇总（`✓/✗ name`）；`tests_failed > 0` → 退出码 1。`--json` 输出 `FileReport`。
- `http-web` / `http-mcp` 后续复用 `run_file`（本 Phase 不改其契约）。

#### 7.9 example fixtures 与 API 替换

`examples.http-client.intellij.net` 实测返回 **503**（2026-09 已下线），按既定决策全部替换为 **httpbin.org**（实测各端点可用且响应结构兼容）：

| 原端点 | 替换为 | 兼容性验证 |
|---|---|---|
| `/ip` | `https://httpbin.org/ip` | `{"origin": "…"}` |
| `/get` | `https://httpbin.org/get` | 含 `headers` 键（满足 Scripts:38 断言）、`Content-Type: application/json` |
| `/post` | `https://httpbin.org/post` | JSON → `.json`（满足 `$.json.balance` / `$.json.firstName`）、form → `.form`、文件 → `.files` |
| `/anything` | `https://httpbin.org/anything` | 回显 query/headers/body |
| `/cookies` | `https://httpbin.org/cookies` | `{"cookies": {…}}` |
| `/status/{200,301,404}` | `https://httpbin.org/status/{200,301,404}` | 状态码一致 |
| HTTP/2 | `https://httpbin.org/get HTTP/2` | httpbin 支持 h2 |

需新建的 fixtures（example 中被引用但缺失）：

- `example/http-client.env.json` — `{ "dev": { "host": "https://httpbin.org", "show_env": "1", "clients": [ …3 条… ] } }`（供 Loop 第二个请求与 GET 环境变量请求使用）
- `example/http-client.private.env.json` — `{ "dev": { "secret": "…" } }`（供 Scripts HMAC 请求使用）
- `example/request-form-data.json` — multipart `< ./request-form-data.json` 引用体
- `example/my-utils.js` — `export function makeSignature()` / `export function findSignature(body)`（供 Scripts ESM import 使用）

#### 7.10 测试策略

- **离线为主**：httpmock 提供确定性响应，覆盖 G1–G24 每一项（详见 [TDD.md](./TDD.md) P7-1~P7-13）。
- **公网 smoke**：`tests/public_smoke_test.rs` 全部标 `#[ignore]`，`cargo test -- --ignored` 才跑，直连 httpbin.org 验证四个 example 端到端。
- **快照**：新增 parser 特性（tags / 前置脚本 / 多行 handler / `>>`）用 insta 快照锁定 AST。

#### 7.11 明确范围外

`sleep(ms)` / `await` / `setTimeout`（官方"执行延迟"文档特性）、`@no-log` 的日志脱敏落盘、gRPC/WebSocket 请求——四个 example 均未使用，不在本 Phase。

#### 7.12 实施结果（Phase 7 已落地）

G1–G24 全部实现，四个 example 端到端跑通：

- **测试**：`cargo test --workspace --all-features` 全绿；`cargo clippy --workspace --all-features --all-targets` 零警告。
  - `example_files_test.rs`（9）：解析级 + httpmock 离线运行级
  - `public_smoke_test.rs`（8，`#[ignore]`）：实连 httpbin.org 全通过（含真实 HTTP/2 over TLS+ALPN）
  - `parser_test.rs`（44，含 P7-2~P7-6 doc tags / 裸 HTTP 版本 / `>>` / 前置脚本 / 多行 handler）、`lexer_test.rs`（28，含 P7-1 `//` 注释）
  - `runner_test.rs`（11）、`cli_test.rs`（19，含全文件执行与失败退出码）
- **实施中新发现的缺口（已修）**：
  1. **host 变量自带 scheme**：JetBrains 惯例把 scheme 放进 host 变量（`GET {{host}}/get`，`host = "https://httpbin.org"`）。
     `execute.rs::build_url` 原先先拼默认 `http://` 再做变量替换，得到 `http://http://…`。
     现改为替换后对 authority 做 `split_once("://")`，前缀通过 `is_scheme`（RFC 3986：首字符 alpha，其余 alnum/`+`/`-`/`.`）校验时内嵌 scheme 优先于请求行/默认值；
     单测 `host_variable_may_carry_its_own_scheme` 覆盖三种情形。
  2. **`consume_headers` 吞掉 `< file` body 行**：header 收集循环原先只在行首为 `> ` / `<> ` / `>>` / `> {%` 时才 break，
     导致与请求行之间没有空行的 `< ./body.json`（无冒号）被当作非法 header 静默丢弃，body 变 `None`。
     现 break 条件放宽为 `text.starts_with('>') || text.starts_with('<')`（header 不可能以这两字符开头），
     使 `< file` 与各类 trailer 行正确交给 `parse_simple_body` / `parse_trailers`；
     回归测试 `body_and_trailer_lines_survive_a_missing_blank_line` 锁定无空行时 FileRef + Inline handler + `>>` redirect 三者均不被吞。
- **CLI 实跑核对**（`--env dev`，在 `/tmp` 副本上执行以免污染仓库）：GET 12 请求（`@no-redirect` 保留 301，`.http-history/` 两个落盘文件）、POST 4×200、Loop 2 请求×3 迭代（6 tests passed，exit 0）、Scripts 6 passed / 1 failed（"Failed test" 为官方示例的预期失败，exit 1）。

## 9. 待定事项（来自 spec 的 TODO）

| spec 章节 | TODO | 本计划暂定方案 |
|---|---|---|
| 4 | 请求执行过程整体描述 | 见 §5.3 执行管线；批量执行见 Phase 7 §7.8 |
| 4.1.1 | 非 ASCII host 处理 | 先 IDNA/punycode，后续按 RFC3986 校准 |
| 4.4 | environment 定义与值插入 | §5.4 约定 + Phase 7 §7.3（分层 Scope、动态变量、env 文件、JSON 值） |
| 4.5 | response handler API | §5.5 最小 API + Phase 7 §7.7（client.test/assert、crypto、jsonPath、request.*、ESM） |
| — | spec 未覆盖的 JetBrains 扩展 | doc tags、前置脚本、`>>` 输出重定向、JSONPath 循环：见 Phase 7 §7.0 差距表 |

## 10. 风险

- **JS 引擎体积**：rquickjs 引入 QuickJS C 代码，可能影响“小体积”目标；需在 Phase 3 评估实际增量，必要时切到 boa（纯 Rust）或把 handler 移到 `http-web` 侧执行。
- **spec TODO 不确定性**：4.4 / 4.5 语义未定，API 设计可能后续与上游规范冲突；采用最小可用集，保留扩展空间。
- **双向编辑同步**：CodeMirror 文本 ↔ 表单双向同步是已知复杂点（dbx 表结构编辑器同款问题），Phase 4 需专门设计同步算法。
- **unicode 路径编码**：spec“已编码不二次编码”规则需仔细测试 `%2520` 这类边界。
