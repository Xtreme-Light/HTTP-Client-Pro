# HTTP Client Pro

支持 [HTTP Request in Editor 格式](./spec.md)（`.http` / `.rest`）的 HTTP 客户端工具，对标 JetBrains HTTP Client / VS Code REST Client。采用"单核多端"架构，一个 Rust 核心库同时覆盖 CLI、Web/Docker、MCP 三种交付场景。

## 功能特性

- 解析 `.http` 文件，支持多个请求以 `###` 分隔
- 全 HTTP method（GET/HEAD/POST/PUT/DELETE/CONNECT/PATCH/OPTIONS/TRACE）
- 环境变量 `{{var}}` 替换（path / header / body）
- 消息体文件引用 `< ./body.json` 与响应引用 `<> response.json`
- Multipart `form-data` 自动拼装
- 响应处理脚本 `> {% ... %}`（QuickJS / ES5.1 沙箱执行）
- 请求命名（`### 登录`）与按名选择执行
- lenient 解析：单请求出错不阻断后续请求

## 架构

```
http_client_pro/
├── crates/
│   ├── http-core/   # 共享核心：lexer + parser + executor + JS handler
│   ├── http-cli/    # 命令行工具
│   ├── http-web/    # axum Web 服务（REST + SSE + OpenAPI）
│   └── http-mcp/   # MCP server（rmcp，权限分层）
├── apps/desktop/    # Vue 3 + Tauri 桌面端
│   ├── src/         # Vue 3 前端源码
│   └── src-tauri/   # Tauri Rust 后端
├── spec.md          # 格式规范
├── plan.md          # 实施计划
├── TDD.md           # 测试清单
└── Dockerfile       # Web 端 Docker 镜像
```

## 快速开始

### 前置要求

- Rust 1.88+（`rustup`）
- 可选：Docker（用于 Web 端容器化部署）

### 构建

```bash
# 构建 workspace 全部 crate
cargo build --workspace

# 构建 release 二进制（启用 LTO + strip，单二进制约 6.6 MB）
cargo build --release
```

### 运行测试

```bash
cargo test --workspace          # 全部测试（186 tests / 22 suites）
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

## CLI 用法

```bash
# 执行 .http 文件中的第一个请求
cargo run -p http-cli -- run req.http

# 按名称或 1-based 索引选择请求
cargo run -p http-cli -- run req.http --request login
cargo run -p http-cli -- run req.http --request 2

# 切换环境（加载 http-client.env.json）
cargo run -p http-cli -- run req.http --env staging

# JSON 输出（便于脚本消费）
cargo run -p http-cli -- run req.http --json
```

环境文件格式（`http-client.env.json`，兼容 JetBrains HTTP Client）：

```json
{
  "staging": { "host": "staging.example.com", "token": "abc" },
  "prod":    { "host": "api.example.com",    "token": "xyz" }
}
```

`.http` 文件示例：

```http
### 登录
POST http://{{host}}/auth/login
Content-Type: application/json

{ "user": "admin", "pass": "{{token}}" }

### 获取用户
GET http://{{host}}/api/v1/users/me
Authorization: Bearer {{token}}
```

## 桌面端（Tauri）

Vue 3 + CodeMirror 6 + Tauri 2 桌面应用，提供三栏布局（编辑器 | 请求 | 响应）、行内执行按钮、JSON 树查看器、环境变量编辑器等。

### 前置要求

- Rust 1.88+（`rustup`）
- Node.js 20+ 与 pnpm（`npm install -g pnpm`）
- Tauri 2 系统依赖（Linux: `webkit2gtk-4.1`, `libgtk-3-dev`, `librsvg2-dev` 等）

### 开发模式

```bash
cd apps/desktop

# 安装前端依赖
pnpm install

# 启动 Tauri 开发模式（自动启动 Vite + Tauri 窗口）
pnpm tauri:dev
# 或
pnpm tauri dev
```

### 纯前端开发模式（不需要 Tauri）

```bash
cd apps/desktop
pnpm install
pnpm dev          # 仅启动 Vite 开发服务器（http://localhost:5173）
```

> 纯前端模式下，HTTP 请求通过 `HttpAdapter` 经 Vite 代理转发到 `http://localhost:8080/execute`，需同时启动 Web 服务：
> ```bash
> cargo run -p http-web --bin http-client-pro-web
> ```

### 构建生产包

```bash
cd apps/desktop
pnpm install
pnpm tauri:build    # 输出到仓库根目录 target/release/bundle/（src-tauri 为 Cargo workspace 成员）
```

### 前端测试

```bash
cd apps/desktop
pnpm test           # 运行 Vitest（101 tests / 8 suites）
```

## Web 服务

启动 axum Web 服务，提供 REST API 与 SSE 流式响应：

```bash
# 开发模式
cargo run -p http-web --bin http-client-pro-web

# 生产模式
HTTP_WEB_ADDR=0.0.0.0:8080 HTTP_WEB_TOKEN=s3cret \
  ./target/release/http-client-pro-web
```

### API 端点

| 方法 | 路径 | 说明 |
|---|---|---|
| `GET` | `/healthz` | 健康检查（无需 auth） |
| `POST` | `/execute` | 执行 `.http` 请求，返回 JSON |
| `GET` | `/sse/execute?src=<http-source>` | SSE 流式执行 |
| `GET` | `/openapi.json` | OpenAPI 3.1 规范 |
| `GET` | `/docs/` | Swagger UI |

配置 `HTTP_WEB_TOKEN` 后，除 `/healthz` 外的所有路由需携带 `Authorization: Bearer <token>`。

### 调用示例

```bash
curl -X POST http://localhost:8080/execute \
  -H "Content-Type: text/plain" \
  -H "Authorization: Bearer s3cret" \
  --data-binary $'GET http://example.com/api\n'
```

## Docker 部署

```bash
docker build -t http-client-pro .
docker run -p 8080:8080 -e HTTP_WEB_TOKEN=s3cret http-client-pro
```

镜像内置 `HEALTHCHECK`，每 30s 探测 `/healthz`。

## MCP Server

供 Claude Code / Cursor / Windsurf 等 AI agent 调用：

```bash
# 默认 safe_write 权限（GET/HEAD/OPTIONS）
cargo run -p http-mcp --bin http-client-pro-mcp

# 完全权限（任意 method）
HTTP_MCP_TIER=high_risk_write cargo run -p http-mcp --bin http-client-pro-mcp
```

权限分层：

| Tier | 允许操作 |
|---|---|
| `read_only` | 仅 `list_requests`，不执行任何请求 |
| `safe_write` | `list_requests` + 执行 GET/HEAD/OPTIONS |
| `high_risk_write` | 全部 HTTP method |

MCP 工具：

- **`list_requests`** — 解析 `.http` 源码，返回请求列表（index / name / method / target）
- **`run_request`** — 执行指定请求，支持环境变量注入与 `{{var}}` 替换

### 客户端配置

以 Claude Code 为例，在 `~/.claude/claude_desktop_config.json` 中添加：

```json
{
  "mcpServers": {
    "http-client-pro": {
      "command": "/path/to/http-client-pro-mcp",
      "env": { "HTTP_MCP_TIER": "safe_write" }
    }
  }
}
```

## 技术栈

| 关注点 | 选型 |
|---|---|
| 工作区 | Cargo workspace（resolver = "2"） |
| 异步运行时 | tokio |
| HTTP 客户端 | reqwest（rustls-tls） |
| URL 处理 | percent-encoding（非 ASCII 编码、已编码不二次编码） |
| JSON | serde / serde_json |
| JS 引擎 | rquickjs（QuickJS 绑定，ES5.1 响应脚本沙箱） |
| Web 框架 | axum 0.8 + tower-http |
| MCP SDK | rmcp 3.x |
| 前端框架 | Vue 3.5 + Pinia + TypeScript |
| 编辑器 | CodeMirror 6（ViewPlugin / Decoration / Gutter） |
| 桌面端 | Tauri 2 |
| 前端测试 | Vitest 4 + happy-dom |

## 模块说明

### http-core

共享核心库，包含：

- **lexer** — 词法分析（spec 第 2 章）：`Separator`、`Comment`、`Content`、`Indented`
- **parser** — 递归下降文法分析（spec 第 3 章），产出 `RequestsFile` AST，带错误恢复
- **execute** — 执行管线（spec 第 4 章）：变量替换 → 编码 → 空白修剪 → multipart 拼装 → URL 构建
- **dispatch** — reqwest 发送 + 响应捕获（status / headers / body / 耗时）
- **handler** — QuickJS 沙箱执行响应脚本 `> {% ... %}`，暴露 `client.global`、`response.*`、`assert` API
- **env** — 环境变量替换 `{{var}}`，保留未定义变量原文 + 诊断

### 响应处理脚本 API

```javascript
> {%
  // 全局变量（跨请求持久化）
  client.global.set("token", response.body.token);
  client.global.get("token");
  client.log("debug message");

  // response 对象
  response.status;                    // number
  response.body;                      // string | object（JSON 自动解析）
  response.headers.value("Content-Type");  // 大小写不敏感

  // 断言
  assert(response.status === 200, "auth failed");
%}
```

## 文档

- [spec.md](./spec.md) — HTTP Request in Editor 格式规范
- [plan.md](./plan.md) — 实施计划与架构决策
- [TDD.md](./TDD.md) — 测试驱动开发清单

## License

双许可：[MIT](./LICENSE-MIT) OR [Apache-2.0](./LICENSE-APACHE)，可任选其一使用。
