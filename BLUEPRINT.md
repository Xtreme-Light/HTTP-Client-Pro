# HTTP Client Pro — 项目蓝图与实现状态

> 生成日期：2026-08-23
> 依据：[plan.md](./plan.md) 实施计划 + [TDD.md](./TDD.md) 测试清单 + 代码库实际状态

## 概览

HTTP Client Pro 是一个支持 HTTP Request in Editor 格式（`.http` / `.rest`）的 HTTP 客户端工具，采用"单核多端"架构：一个 Rust 核心库（`http-core`）同时覆盖 CLI、Web/Docker、MCP、桌面四种交付场景。

| 指标 | 数值 |
|---|---|
| Rust 测试 | 200 tests / 22 suites（全绿） |
| 前端测试 | 51 tests / 8 suites（全绿） |
| Rust crate 数 | 4（http-core、http-cli、http-web、http-mcp） |
| 前端组件数 | 3 + 4 stores + 4 lib 模块 |
| Release 二进制 | 1（`http-client-pro-web`，6.6 MB） |
| 代码格式化 / Lint | `cargo fmt` + `cargo clippy -D warnings` + `vitest` 全绿 |

---

## Phase 0 — 脚手架（基线）

| 项目 | 状态 | 说明 |
|---|---|---|
| Cargo workspace（resolver = "2"） | ✅ 已完成 | 4 个成员 crate |
| pnpm workspace | ✅ 已完成 | `apps/desktop` |
| `http-core` crate 骨架 | ✅ 已完成 | lexer + parser + execute + dispatch + handler + env |
| `apps/desktop` Vue 3 + Vite | ✅ 已完成 | Vue 3.5 + Vite 8 + TypeScript 6 |
| Release profile 体积优化 | ✅ 已完成 | `lto` + `strip` + `opt-level=s` + `codegen-units=1` |
| `src-tauri` Tauri v2 应用 | ❌ 未实现 | plan.md 规划但未创建 `src-tauri/` 目录 |
| CI（lint/test/build） | ❌ 未实现 | 无 `.github/workflows/` |

---

## Phase 1 — Parser（spec 第 2~3 章）

| 项目 | 状态 | 说明 |
|---|---|---|
| Lexer — token 流（spec 2.1~2.5） | ✅ 已完成 | `Separator`/`Comment`/`Content`/`Indented`/`Empty` |
| 递归下降 Parser → AST | ✅ 已完成 | `RequestsFile { requests, diagnostics }` |
| Request line（method + target） | ✅ 已完成 | 支持 origin-form / absolute-form / asterisk-form |
| Headers（含续行） | ✅ 已完成 | `field-name ':' ows field-value ows` |
| Message body（in-place + `< file`） | ✅ 已完成 | 文件引用 + 内联 body |
| Multipart form-data | ✅ 已完成 | boundary 切分 + headers + body parts |
| Response handler `> {% ... %}` | ✅ 已完成 | 脚本 + 文件引用两种来源 |
| Response ref `<> file` | ✅ 已完成 | 响应持久化路径 |
| Environment variable `{{var}}` | ✅ 已完成 | 解析 + 替换 + 未定义诊断 |
| 请求命名 `### name` | ✅ 已完成 | 分隔符注释 → `Request.name` |
| Insta 快照覆盖 | ✅ 已完成 | spec 所有 `__Example:` 用例 |
| 错误诊断 + 错误恢复 | ✅ 已完成 | 跳过 malformed 请求、记录 diagnostics |
| 多行续行（path/query/header） | ✅ 已完成 | `new-line-with-indent` 续行拼接 |

---

## Phase 2 — Executor（spec 第 4.1~4.3）

| 项目 | 状态 | 说明 |
|---|---|---|
| 变量替换 `{{var}}` | ✅ 已完成 | path / header / body 全覆盖 |
| 编码 — path/query 非 ASCII | ✅ 已完成 | percent-encode 非 ASCII，已编码 `%xx` 不二次编码 |
| 编码 — host | ⚠️ 部分 | IDNA/punycode 待实现（spec 4.1.1 标记 TODO） |
| 空白修剪 | ✅ 已完成 | path/query 每行首尾 trim；body 整体首尾 trim |
| Multipart 拼装 | ✅ 已完成 | boundary + headers + body parts |
| Dispatch（reqwest） | ✅ 已完成 | status / headers / body / elapsed / url 捕获 |
| 流式响应捕获 | ✅ 已完成 | reqwest stream body |

---

## Phase 3 — Response Handler（spec 第 4.5）

| 项目 | 状态 | 说明 |
|---|---|---|
| rquickjs 沙箱集成 | ✅ 已完成 | ES5.1 脚本执行，feature `js-handler` |
| `client.global.set/get` | ✅ 已完成 | 跨请求持久化 |
| `client.log` | ✅ 已完成 | 日志输出 |
| `response.status` | ✅ 已完成 | number |
| `response.body` | ✅ 已完成 | string + JSON 自动解析为 object |
| `response.headers.value(name)` | ✅ 已完成 | 大小写不敏感 |
| `assert(condition, message)` | ✅ 已完成 | 失败生成诊断 + 不阻断后续请求 |
| 文件引用脚本 `> script.js` | ✅ 已完成 | 从文件加载执行 |
| 脚本超时中断 | ✅ 已完成 | `Runtime::set_interrupt_handler` |
| 集成场景（auth → token → 后续请求） | ✅ 已完成 | handler 修改后续请求行为 |

---

## Phase 4 — 前端 MVP

| 项目 | 状态 | 说明 |
|---|---|---|
| CodeMirror 6 编辑器 | ✅ 已完成 | StreamLanguage `.http` 语法高亮 |
| `###` 分隔符 token 化 | ✅ 已完成 | `separator` tag |
| `#` / `//` 注释高亮 | ✅ 已完成 | `comment` tag |
| Method 高亮 | ✅ 已完成 | `method` tag + CSS class |
| `{{var}}` 装饰 | ✅ 已完成 | ViewPlugin — 已定义绿色 / 未定义红色波浪线 |
| Run 按钮 + `Cmd/Ctrl+Enter` | ✅ 已完成 | keymap `Mod-Enter` → `useRunCurrent.run()` |
| 响应面板 | ✅ 已完成 | status chip + elapsed + headers 表 + JSON pretty-print |
| 环境切换器 | ✅ 已完成 | 下拉选择 dev/staging |
| Pinia stores | ✅ 已完成 | environment / request / response / history |
| 平台适配 `lib/backend/` | ✅ 已完成 | `http.ts`（REST + SSE）+ `tauri.ts`（IPC 桩）+ `detectAdapter()` |
| 桌面端 Tauri IPC 打通 | ❌ 未实现 | 适配层已预留，但 `src-tauri/` 未创建 |
| 请求面板表单（双向同步） | ❌ 未实现 | plan.md §6 规划的"method/target/headers/body tabs"未做 |
| Tailwind CSS 4 + shadcn-vue | ❌ 未实现 | 使用 scoped CSS 替代 |
| 9 主题切换 | ❌ 未实现 | `@uiw/codemirror-theme-*` 未集成 |
| JSON 树查看 + shiki 高亮 | ❌ 未实现 | 当前用 `JSON.stringify` 简单格式化 |
| ECharts 耗时图表 | ❌ 未实现 | |
| vue-virtual-scroller | ❌ 未实现 | 大响应体浏览未做 |
| i18n 中/英 | ❌ 未实现 | `vue-i18n` 未集成 |
| oxlint + husky | ❌ 未实现 | Lint 链未配置 |

---

## Phase 5 — 多端

| 项目 | 状态 | 说明 |
|---|---|---|
| `http-web` axum REST API | ✅ 已完成 | `POST /execute` + `GET /sse/execute` + `GET /healthz` |
| OpenAPI 生成 | ✅ 已完成 | 手写 `serde_json::Value`（未用 utoipa 派生） |
| Swagger UI | ✅ 已完成 | `/docs/` 路由 |
| Bearer token 鉴权 | ✅ 已完成 | `HTTP_WEB_TOKEN` 环境变量 |
| 前端 `lib/backend/http.ts` 适配 | ✅ 已完成 | fetch POST + SSE stream parser |
| Docker 镜像 | ✅ 已完成 | 多阶段 Dockerfile + HEALTHCHECK |
| `http-cli` 命令行工具 | ✅ 已完成 | `run` 子命令 + `--request` / `--env` / `--json` |
| `http-mcp` MCP server | ✅ 已完成 | rmcp 3.x + `list_requests` / `run_request` 工具 |
| MCP 权限分层 | ✅ 已完成 | `read_only` / `safe_write` / `high_risk_write` |
| npm 包分发 | ❌ 未实现 | `packages/` 目录未创建 |
| Tauri 安装包（NSIS/pkg/AppImage） | ❌ 未实现 | `src-tauri/` 未创建 |

---

## Phase 6 — 打磨

| 项目 | 状态 | 说明 |
|---|---|---|
| 历史与重放 | ⚠️ 部分 | `history` store 有 localStorage 持久化，但 UI 未展示历史列表/重放按钮 |
| 响应 diff（`<> ref`） | ❌ 未实现 | |
| 配置加密导入/导出 | ❌ 未实现 | `config.rs` + `configCrypto.ts` 未做 |
| 自动更新（Tauri updater） | ❌ 未实现 | |
| i18n 中/英 | ❌ 未实现 | |
| 性能基准 | ❌ 未实现 | |

---

## crate 模块清单

### http-core（共享核心）

| 文件 | 职责 | 测试数 |
|---|---|---|
| [lexer.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-core/src/lexer.rs) | 词法分析：line 分类 | 17 |
| [parser.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-core/src/parser.rs) | 递归下降 → AST + 错误恢复 | 15 |
| [model.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-core/src/model.rs) | AST 类型定义 | — |
| [execute.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-core/src/execute.rs) | 变量替换 + 编码 + 空白 + multipart | 13 |
| [dispatch.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-core/src/dispatch.rs) | reqwest 发送 + 响应捕获 | 23 |
| [handler.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-core/src/handler.rs) | QuickJS 沙箱执行响应脚本 | 19 |
| [env.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-core/src/env.rs) | 环境变量替换 | — |
| [error.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-core/src/error.rs) | 错误类型 | — |

### http-web（axum Web 后端）

| 文件 | 职责 | 测试数 |
|---|---|---|
| [routes.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-web/src/routes.rs) | REST + SSE + OpenAPI + Swagger | 9 |
| [state.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-web/src/state.rs) | AppState + ServerBuilder | — |
| [error.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-web/src/error.rs) | ApiError → HTTP 映射 | — |
| [main.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-web/src/main.rs) | 生产二进制入口 | — |

### http-mcp（MCP server）

| 文件 | 职责 | 测试数 |
|---|---|---|
| [permissions.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-mcp/src/permissions.rs) | 三层权限：ReadOnly / SafeWrite / HighRiskWrite | 5 |
| [tools.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-mcp/src/tools.rs) | `list_requests` + `run_request` 工具 | 11 |
| [server.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-mcp/src/server.rs) | rmcp `#[tool_router]` + stdio | — |
| [main.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-mcp/src/main.rs) | MCP 二进制入口 | — |

### http-cli（命令行工具）

| 文件 | 职责 | 测试数 |
|---|---|---|
| [lib.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-cli/src/lib.rs) | `run` 子命令 + 参数解析 + 环境加载 | 17 |
| [main.rs](file:///home/whate/person_workspaces/http_client_pro/crates/http-cli/src/main.rs) | CLI 二进制入口 | — |

### apps/desktop（Vue 3 前端）

| 文件 | 职责 |
|---|---|
| [App.vue](file:///home/whate/person_workspaces/http_client_pro/apps/desktop/src/App.vue) | CSS Grid 三栏布局 |
| [components/EditorPane.vue](file:///home/whate/person_workspaces/http_client_pro/apps/desktop/src/components/EditorPane.vue) | CodeMirror 6 宿主 + Mod-Enter + 变量装饰 |
| [components/RequestToolbar.vue](file:///home/whate/person_workspaces/http_client_pro/apps/desktop/src/components/RequestToolbar.vue) | method chip + Run 按钮 + 环境切换器 |
| [components/ResponsePane.vue](file:///home/whate/person_workspaces/http_client_pro/apps/desktop/src/components/ResponsePane.vue) | status chip + headers 表 + JSON body |
| [stores/environment.ts](file:///home/whate/person_workspaces/http_client_pro/apps/desktop/src/stores/environment.ts) | 环境变量管理 |
| [stores/request.ts](file:///home/whate/person_workspaces/http_client_pro/apps/desktop/src/stores/request.ts) | .http 源码 + blocks + currentBlock |
| [stores/response.ts](file:///home/whate/person_workspaces/http_client_pro/apps/desktop/src/stores/response.ts) | 执行结果 |
| [stores/history.ts](file:///home/whate/person_workspaces/http_client_pro/apps/desktop/src/stores/history.ts) | 历史记录 + localStorage |
| [lib/parse.ts](file:///home/whate/person_workspaces/http_client_pro/apps/desktop/src/lib/parse.ts) | .http block 分割器 |
| [lib/backend/http.ts](file:///home/whate/person_workspaces/http_client_pro/apps/desktop/src/lib/backend/http.ts) | REST + SSE 适配器 |
| [lib/backend/tauri.ts](file:///home/whate/person_workspaces/http_client_pro/apps/desktop/src/lib/backend/tauri.ts) | Tauri IPC 适配器（桩） |
| [lib/codemirror/lang-http.ts](file:///home/whate/person_workspaces/http_client_pro/apps/desktop/src/lib/codemirror/lang-http.ts) | StreamLanguage .http 语法 |
| [lib/codemirror/extensions.ts](file:///home/whate/person_workspaces/http_client_pro/apps/desktop/src/lib/codemirror/extensions.ts) | HighlightStyle + 变量装饰 |

---

## 未实现功能优先级排序

### 高优先级（核心体验缺口）

1. **Tauri 桌面壳**（`src-tauri/`） — 前端目前依赖 http-web 后端运行，无法作为独立桌面应用分发
2. **CI 流水线** — 无 GitHub Actions，无法自动化 lint/test/build
3. **历史面板 UI** — store 已有持久化逻辑，但前端无历史列表/重放 UI
4. **请求面板表单** — plan.md §6 规划的 method/target/headers/body tabs 双向同步

### 中优先级（用户体验增强）

5. **Tailwind CSS + shadcn-vue** — 当前 scoped CSS 够用但扩展性差
6. **JSON 树查看 + shiki 高亮** — 当前 `JSON.stringify` 格式化不够友好
7. **9 主题切换** — `@uiw/codemirror-theme-*`
8. **i18n 中/英** — `vue-i18n`
9. **host 编码**（spec 4.1.1 TODO） — IDNA/punycode

### 低优先级（打磨与分发）

10. **npm 包分发** — `packages/cli` + `packages/mcp-server`
11. **响应 diff** — `<> ref` 历史对比
12. **配置加密** — `config.rs` + `configCrypto.ts`
13. **自动更新** — Tauri updater + minisign
14. **ECharts 耗时图表** — 响应耗时直方图
15. **vue-virtual-scroller** — 大响应体浏览
16. **oxlint + husky** — Lint 链
17. **性能基准** — 大 `.http` 文件、大响应体

---

## 启动方式

```bash
# Rust 测试
cargo test --workspace           # 200 tests 全绿
cargo clippy --workspace -- -D warnings

# 前端测试
cd apps/desktop && npx vitest run  # 51 tests 全绿

# Web 后端
cargo run -p http-web --bin http-client-pro-web  # :8080

# 前端 dev server
pnpm dev  # :5173（proxy → :8080）

# CLI
cargo run -p http-cli -- run req.http --request login --env staging --json

# MCP server
HTTP_MCP_TIER=safe_write cargo run -p http-mcp --bin http-client-pro-mcp

# Docker
docker build -t http-client-pro .
docker run -p 8080:8080 -e HTTP_WEB_TOKEN=s3cret http-client-pro
```
