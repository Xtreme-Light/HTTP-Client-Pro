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

## 9. 待定事项（来自 spec 的 TODO）

| spec 章节 | TODO | 本计划暂定方案 |
|---|---|---|
| 4 | 请求执行过程整体描述 | 见 §5.3 执行管线 |
| 4.1.1 | 非 ASCII host 处理 | 先 IDNA/punycode，后续按 RFC3986 校准 |
| 4.4 | environment 定义与值插入 | 见 §5.4 约定 |
| 4.5 | response handler API | 见 §5.5 最小 API |

## 10. 风险

- **JS 引擎体积**：rquickjs 引入 QuickJS C 代码，可能影响“小体积”目标；需在 Phase 3 评估实际增量，必要时切到 boa（纯 Rust）或把 handler 移到 `http-web` 侧执行。
- **spec TODO 不确定性**：4.4 / 4.5 语义未定，API 设计可能后续与上游规范冲突；采用最小可用集，保留扩展空间。
- **双向编辑同步**：CodeMirror 文本 ↔ 表单双向同步是已知复杂点（dbx 表结构编辑器同款问题），Phase 4 需专门设计同步算法。
- **unicode 路径编码**：spec“已编码不二次编码”规则需仔细测试 `%2520` 这类边界。
