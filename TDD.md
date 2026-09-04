# TDD.md — 测试驱动开发清单

> 依据：[plan.md](./plan.md)、[spec.md](./spec.md)
> 原则：每写一行实现，先写一个失败测试；红 → 绿 → 重构。
> 测试栈：Rust 侧 `cargo test` + `insta` 快照；前端 Vitest + happy-dom。

## 测试分层

| 层级 | 范围 | 工具 |
|---|---|---|
| 单元 | lexer / parser / executor / handler 各函数 | `cargo test`、insta |
| 文法用例 | spec 中所有 `__Example:__` 与反例 | insta 快照 + 数据驱动 |
| 集成 | AST → 执行 → 响应 端到端 | `cargo test` + mock server（`httpmock`） |
| 前端单元 | store / composable / 工具函数 | Vitest |
| 前端组件 | 编辑器、请求/响应面板 | Vitest + @vue/test-utils |
| 端到端 | Tauri 命令、Web 路由、MCP 工具 | Vitest 集成 + Tauri mock |

---

## Phase 0 — 脚手架（基线测试）

### P0-1 工作区可构建
- [ ] `cargo build --workspace` 成功
- [ ] `cargo test --workspace` 通过（占位 `tests it_works`）
- [ ] `pnpm install` 成功
- [ ] `pnpm build` 成功（desktop bundle 可生成）
- [ ] `cargo fmt --check` 与 `cargo clippy -- -D warnings` 通过

### P0-2 crate 骨架
- [ ] `http-core` lib 可被 `http-cli`、`http-web`、`http-mcp`、`src-tauri` 依赖
- [ ] release profile 配置生效（`opt-level=s`、`lto=true`）

---

## Phase 1 — Parser（spec 第 2~3 章）

### P1-1 Lexer token（spec 2.1~2.3）
- [ ] `alpha`：识别 A-Z / a-z
- [ ] `digit`：识别 0-9
- [ ] `identifier`：含 `alpha`、`digit`、`-`、`_`
- [ ] `input-character`：接受任意 unicode（中文、emoji）非 new-line
- [ ] `new-line`：分别识别 LF / CR / CRLF 三种
- [ ] `new-line-with-indent`：new-line 后跟 required-whitespace
- [ ] `whitespace`：SP / HT / FF
- [ ] `optional-whitespace` / `required-whitespace`

### P1-2 Lexer comment & separator（spec 2.4~2.5）
- [ ] `line-comment`：`#` 开头至行尾
- [ ] `line-comment`：`//` 开头至行尾
- [ ] `request-separator`：`###` 至行尾
- [ ] `request-separator`：`###` 带尾部注释 `### request comment`

### P1-3 Requests file（spec 3.1）
- [ ] 单请求无分隔符
- [ ] 多请求以 `###` 分隔
- [ ] 文件首尾允许多个 `###`
- [ ] 空/纯注释文件返回空 `RequestsFile`
- [ ] 连续多个 `###` 视为单分隔符

### P1-4 Request line（spec 3.2.1）
- [ ] 缺省 method 默认 `GET`
- [ ] 全部 9 个 method 识别：GET/HEAD/POST/PUT/DELETE/CONNECT/PATCH/OPTIONS/TRACE
- [ ] 缺省 http-version
- [ ] `HTTP/1.1` / `HTTP/2.0` 识别
- [ ] 非法 method（如 `FOO`）报错带位置

### P1-5 Request target（spec 3.2.1.1~3.2.1.4）
- [ ] `origin-form`：`/api/get` + 必须有 `Host` 头
- [ ] `absolute-form`：`http://example.com/api/get`
- [ ] scheme 缺省 `http`
- [ ] `https` scheme 识别
- [ ] `asterisk-form`：`*`
- [ ] authority：`host`、`host:port`、`[::1]`、`[::1]:8080`、`127.0.0.1:8080`
- [ ] `absolute-path` 多行续行（new-line-with-indent）拼接
- [ ] `query`：`?id=42` + 多行续行
- [ ] `fragment`：`#q=hello+world`
- [ ] unicode path / query / fragment（中文）

### P1-6 Headers（spec 3.2.2）
- [ ] 单 header：`From: user@example.com`
- [ ] 多 header
- [ ] field-value 前后 optional-whitespace 被 trim
- [ ] field-value 多行续行（new-line-with-indent）拼接
- [ ] field-name 含任意字符（除 `:`）
- [ ] 大小写不敏感（保存原值，比较时小写）

### P1-7 Message body（spec 3.2.3）
- [ ] in-place JSON body
- [ ] body 与 headers 之间必须空行
- [ ] in-place body 首尾空白被 trim（spec 4.2.2）
- [ ] `input-file-ref`：`< ./input.json` 引用外部文件
- [ ] file body 不 trim
- [ ] body 终止于 `<`、`<> `、`###`

### P1-8 Multipart form-data（spec 3.2.3.1）
- [ ] 单 multipart-field 含 headers + body
- [ ] 多 field 用 boundary 切分
- [ ] 结尾 boundary `--abcd--`
- [ ] field body 可为 `input-file-ref`
- [ ] 缺 `Content-Type: multipart/form-data; boundary=...` 报错

### P1-9 Response handler（spec 3.2.4）
- [ ] in-place：`> {% client.global.set("auth", response.body.token); %}`
- [ ] file ref：`> ./handler.js`
- [ ] in-place 不允许含 `%}`
- [ ] in-place 不允许含 `###`

### P1-10 Response ref（spec 3.2.5）
- [ ] `<> previous-response.200.json` 识别为 ResponseRef

### P1-11 Environment variables（spec 3.2.6）
- [ ] `{{host}}` 识别
- [ ] `{{ host }}` 带空白
- [ ] `{{ element-id }}` identifier 含 `-`
- [ ] 大小写敏感
- [ ] 出现在 path / header / body
- [ ] 未闭合 `{{host` 报错

### P1-12 Parser 错误恢复
- [ ] 单个请求解析失败不阻断后续请求
- [ ] 诊断含 line/col 与错误类别
- [ ] 全部 spec `__Example:__` 通过 insta 快照

---

## Phase 2 — Executor（spec 第 4.1~4.3 章）

### P2-1 变量替换（spec 4.4）
- [ ] `{{host}}` 替换为环境值
- [ ] 优先级：请求级 > 环境级 > 全局级
- [ ] 未定义变量保留原文 + 警告
- [ ] 变量值不二次编码
- [ ] 替换范围限于 path / header / body

### P2-2 编码（spec 4.1）
- [ ] path 非 ASCII percent-encode（`中文` → `%E4%B8%AD%E6%96%87`）
- [ ] query 非 ASCII 编码
- [ ] 已编码 `%20` 不二次编码
- [ ] 用户写 `%2520` 发出为 `%20`
- [ ] body 按 `Content-Type` 编码，缺省 UTF-8
- [ ] host 非 ASCII：IDNA/punycode 处理

### P2-3 空白修剪（spec 4.2）
- [ ] path 多行续行首尾 trim → `http://example.com/api/get`
- [ ] path 含 `%20api%20` 保留编码空格
- [ ] in-place body 整体首尾 trim → 仅 `message-body`
- [ ] file body 不 trim → `\nmessage-body\n` 完整发送

### P2-4 Multipart dispatch（spec 4.3）
- [x] 无 `filename` 的 part 作 string 发送
- [x] 有 `filename` 的 part 作 file 发送
- [x] boundary 正确拼装

### P2-5 执行集成（httpmock）
- [x] GET 200 → status/headers/body 捕获
- [x] POST JSON body 透传
- [x] multipart body 透传
- [x] 流式响应（chunked）捕获
- [x] 超时 / 连接错误 → 错误诊断
- [x] 请求耗时记录

### P2-6 Response ref 落盘
- [x] `<> path` 写入响应文件
- [x] 已存在文件可覆盖（按策略）

---

## Phase 3 — Response Handler（spec 第 4.5）

### P3-1 JS 沙箱
- [x] rquickjs 引擎初始化（feature `js-handler`）
- [x] 执行 `client.global.set("k","v")` 不抛错
- [x] 脚本运行超时终止
- [x] 脚本异常捕获为诊断

### P3-2 client API
- [x] `client.global.set` / `get` 跨请求持久化
- [x] `client.log` 输出捕获到日志
- [x] `client.assert(condition, msg)` 失败转诊断

### P3-3 response API
- [x] `response.status` 数字
- [x] `response.body` string（无 Content-Type）
- [x] `response.body` object（JSON 自动解析）
- [x] `response.headers.value(name)` 字符串
- [x] `response.headers.valueOf` 大小写不敏感

### P3-4 assert
- [x] `assert(status === 200, "ok")` 通过
- [x] assert 失败生成失败诊断 + 不阻断后续请求

### P3-5 file handler
- [x] `> ./handler.js` 从文件加载脚本
- [x] 文件不存在报错

### P3-6 集成
- [x] auth 请求 set token → 后续请求 header 引用 `{{token}}`
- [x] handler 修改后续请求行为

---

## Phase 4 — 前端 MVP

### P4-1 CodeMirror 语言包
- [x] `###` 分隔符 token 化（StreamLanguage `separator` tag）
- [x] `#` / `//` 注释 token（`comment` tag）
- [x] method 高亮（`method` tag + CSS class）
- [x] `{{var}}` 装饰（ViewPlugin + 已定义/未定义 CSS class）
- [x] 语法高亮快照（Vitest — 8 suites / 51 tests 全绿）

### P4-2 store
- [x] environment store 切换环境（dev/staging 默认 + addEnv/setVar/resolve）
- [x] request store 解析 `.http` 文本 → blocks（splitRequests + currentBlock getter）
- [x] response store 接收执行结果（start/ok/fail/reset）
- [x] history store 持久化（localStorage + serialize/deserialize 可独立测试）

### P4-3 组件
- [x] 编辑器组件渲染文本（EditorPane.vue — CodeMirror 6 宿主）
- [x] Run 按钮触发当前请求执行（RequestToolbar.vue — POST /execute 端到端验证通过）
- [x] `Cmd+Enter` 快捷键（CodeMirror keymap.of Mod-Enter → useRunCurrent.run()）
- [x] 响应面板渲染 status/headers/body（ResponsePane.vue — status chip + headers table + JSON pretty-print）
- [x] 环境切换器下拉（EnvSwitcher 集成在 RequestToolbar 内）

### P4-4 平台适配
- [x] `tauri.ts` 调用 `invoke`（动态 import @tauri-apps/api/core + 可注入 mock invoke）
- [x] `http.ts` 调用 fetch + SSE（POST /execute + SSE stream parser + Bearer auth）
- [x] 接口签名一致（BackendAdapter 接口 + detectAdapter 工厂 + 类型测试）

---

## Phase 5 — 多端

### P5-1 http-web（axum）
- [x] `POST /execute` 执行请求
- [x] `GET /sse/execute` 流式响应
- [x] OpenAPI 生成（utoipa）— 手写 `serde_json::Value`，避免 utoipa 派生耦合
- [x] 鉴权（密码 / token）

### P5-2 http-cli
- [x] `http-client-pro run file.http` 执行首个请求
- [x] `--request <name>` 指定请求（按 `### name` 分隔符匹配，或 1-based 索引）
- [x] `--env staging` 切换环境（JetBrains `http-client.env.json` 格式）
- [x] `--json` 输出 JSON 结果（默认人类可读）

### P5-3 http-mcp（rmcp）
- [x] `list_requests` 工具列出文件请求
- [x] `run_request` 工具执行
- [x] `read_only` 模式禁止写方法
- [x] `safe_write` 允许 GET/HEAD/OPTIONS
- [x] `high_risk_write` 允许全方法

### P5-4 Docker
- [x] 镜像构建（多阶段 Dockerfile，release profile，最终 `debian:bookworm-slim`）
- [x] 健康检查端点（`GET /healthz`，无需 auth）

---

## Phase 6 — 打磨

### P6-1 历史
- [ ] 执行后入历史
- [ ] 重放恢复请求
- [ ] 历史搜索

### P6-2 响应 diff
- [ ] `<> ref` 命中时展示 diff
- [ ] JSON body 结构 diff

### P6-3 配置加密
- [ ] 导出加密配置
- [ ] 导入解密配置
- [ ] 桌面 safeStorage 路径

### P6-4 性能
- [ ] 大 `.http` 文件（10k 请求）解析 < 1s
- [ ] 大响应体（100MB）流式渲染不卡顿

---

## 测试数据组织

```
crates/http-core/
├── tests/
│   ├── fixtures/
│   │   ├── lexer/         # P1-1~P1-2 输入片段
│   │   ├── parser/        # P1-3~P1-11 .http 文件
│   │   ├── executor/      # P2-1~P2-4 输入 + 期望
│   │   └── handler/       # P3-1~P3-6 脚本
│   ├── snapshots/         # insta 自动生成
│   ├── lexer_test.rs
│   ├── parser_test.rs
│   ├── executor_test.rs
│   └── handler_test.rs
└── benches/
    └── parse_large.rs    # P6-4 基准
```

## TDD 流程约定

1. **Red**：写测试，`cargo test` 失败
2. **Green**：最小实现让测试通过
3. **Refactor**：重构并保持绿色
4. **Commit**：每个绿 → 单独 commit（粒度：一个测试用例 + 其实现）
5. **快照**：insta 自动接受前必须人工 review diff
