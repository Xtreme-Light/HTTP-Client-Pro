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

## Phase 7 — JetBrains 官方 example 全兼容

> 差距编号 G1~G24 见 [plan.md](./plan.md) §7.0。测试文件位于 `crates/http-core/tests/`。
> 离线用例用 `httpmock`；公网用例集中在 `public_smoke_test.rs` 并标 `#[ignore]`。

### P7-1 Lexer：`//` 注释（G1）— `lexer_test.rs`（已落地，28 测试全绿）
- [x] `//TIP <p>...</p>` 识别为 `Line::Comment` — `slash_comment_without_space_is_still_a_comment`（+ 既有 `slash_slash_comment_line`）
- [x] `//` 后无空格（`//TIP`）同样识别 — `slash_comment_without_space_is_still_a_comment`
- [x] `###` 优先级高于 `//`（`### //x` 仍是 separator）— `separator_beats_slash_comment`
- [x] 缩进的 `//` 行按 `Line::Indented` 原样保留（Scripts example `{% %}` 块内的 JS 注释不丢行、不被当作文档注释吃掉）— `indented_slash_line_is_indented_not_comment`

### P7-2 Parser：doc tags（G2）— `parser_test.rs`（已落地）
- [x] `# @no-redirect` → `DocTag::NoRedirect`
- [x] `# @no-cookie-jar` → `DocTag::NoCookieJar`
- [x] `# @no-auto-encoding` → `DocTag::NoAutoEncoding`
- [x] `# @no-log` → `DocTag::NoLog`
- [x] `# @timeout 5000` → `DocTag::Timeout{millis:5000}`
- [x] `# @connection-timeout 1000` → `DocTag::ConnectionTimeout{millis:1000}`
  （以上六项集中在 `doc_tags_are_harvested_from_hash_comments`）
- [x] `// @no-redirect`（`//` 风格）同样识别 — `doc_tags_accept_slash_slash_comments`
- [x] tag 作用于其后紧邻的那个请求，不泄漏到下一个 `###` 块 — `doc_tags_apply_only_to_the_request_they_precede`
- [x] 普通注释（`# hello`）不产生 tag — `prose_comments_produce_no_tags`
- [x] `@name My Request` → `DocTag::Name`，并与既有 `### name` 语义一致 — `name_tag_becomes_the_request_name`（`@name` 优先于 `### name`）

### P7-3 Parser：裸 HTTP 版本（G3）— `parser_test.rs`（已落地）
- [x] `GET https://x/y HTTP/2` 解析成功，`http_version == Some("HTTP/2")` — `bare_http_version_forms_are_accepted`
- [x] `HTTP/1.1` / `HTTP/2.0` / `HTTP/3` 均接受 — `bare_http_version_forms_are_accepted`
- [x] `HTTP/x` 仍报错 — `malformed_http_version_is_a_diagnostic`（1 条 Parse 诊断、requests 为空）
- [x] `execute`：`HTTP/2` → `reqwest::Version::HTTP_2` — `dispatch_test.rs::send_options_from_tags_and_http2_version`；公网协商 `smoke_http2_is_negotiated_over_tls`

### P7-4 Parser：输出重定向（G4）— `parser_test.rs` + `dispatch_test.rs`（已落地）
- [x] `>> path.json` → `OutputRedirect{path, force:false}` — `output_redirect_keeps_the_path_verbatim`（镜像 GET.http `>> {{$historyFolder}}/my-response.json`）
- [x] `>>! path.json` → `OutputRedirect{path, force:true}` — `forced_output_redirect_sets_the_force_flag`
- [x] `>>` 行不再被吞入 message body（body 为 `None`）— `output_redirect_keeps_the_path_verbatim`
- [x] `>>` 与 `> {% %}` handler 可共存于同一请求 — `output_redirect_coexists_with_a_response_handler`
- [x] `>>` 与 `<> response-ref` / `< file` 可共存（无空行也不被吞）— `body_and_trailer_lines_survive_a_missing_blank_line`
- [x] 路径中的 `{{$historyFolder}}` 保持原文（替换发生在执行期）— `output_redirect_keeps_the_path_verbatim` + `runner_test.rs::history_folder_redirect_is_rooted_at_the_base_dir`
- [x] 落盘：`force=false` 且文件存在 → 写入 `name-1.json` — `dispatch_test.rs::output_redirect_non_force_appends_numeric_suffix`
- [x] 落盘：`force=true` 且文件存在 → 覆盖原文件 — `dispatch_test.rs::output_redirect_force_overwrites_existing_file`
- [x] 落盘：父目录不存在 → 自动创建 — `dispatch_test.rs::output_redirect_substitutes_env_and_creates_dirs`

### P7-5 Parser：前置请求脚本（G5）— `parser_test.rs`（已落地）
- [x] `< {% ... %}` 单行 → `pre_request_script = Some(Inline{..})` — `single_line_pre_request_script_is_inline`
- [x] `< {%` + 多行 + `%}` → 完整脚本被收集（含空行与缩进）— `multiline_pre_request_script_collects_every_line`（镜像 Loop/Scripts example）
- [x] `< {% ... %}` 不再被误判为 `MessageBody::FileRef` — `single_line_pre_request_script_is_inline`（body == None）
- [x] `< ./body.json` 仍是 `MessageBody::FileRef`（回归）— `angle_bracket_file_ref_is_still_a_body_reference`
- [x] 前置脚本 + 请求行 + body + 响应处理器 四段共存 — `body_and_trailer_lines_survive_a_missing_blank_line` + `example_files_test.rs::script_and_loop_examples_carry_their_pre_and_post_scripts`
- [x] 脚本内 `//` 注释行不被 lexer 提前吃掉（`{% %}` 区块内原样保留）— `multiline_pre_request_script_collects_every_line`（含 `// test data`）+ `lexer_test.rs::indented_slash_line_is_indented_not_comment`

### P7-6 Parser：多行响应处理器（G6）— `parser_test.rs`（已落地）
- [x] `> {%` ⏎ 多行 ⏎ `%}` → `ResponseHandler::Inline{script}` 含全部行 — `multiline_response_handler_collects_every_line`
- [x] 单行 `> {% ... %}` 回归不破 — `single_line_response_handler_is_still_inline`
- [x] 脚本体内的 `%` 与 `{` 不导致提前终止（仅行尾 `%}` 终止）— `multiline_response_handler_collects_every_line`（体含 `100 % 7`、`{a: 1}`）
- [x] `> ./handler.js` 文件引用回归不破 — `handler_file_ref_regression`
- [x] Loop/Scripts 两个 example 的 AST 用直接断言锁定（替代 insta 快照，见 P7-16）— `example_files_test.rs::script_and_loop_examples_carry_their_pre_and_post_scripts`

> 实施中修复的解析器缺口：`consume_headers` 原先只在行首为 `> ` / `<> ` / `>>` / `> {%` 时才停止收集 header，
> 导致紧随请求行、与请求行之间没有空行的 `< ./body.json`（无冒号）被当作非法 header 静默丢弃（`parse_header_field` 返回 `None`），body 变成 `None`。
> 修复：break 条件放宽为 `text.starts_with('>') || text.starts_with('<')`（header 不可能以这两个字符开头，安全），
> 使 `< file` body 起始行与各类 trailer 行都能正确交给 `parse_simple_body` / `parse_trailers`。
> 回归测试 `body_and_trailer_lines_survive_a_missing_blank_line` 锁定「无空行时 FileRef + Inline handler + `>>` redirect 三者均不被吞」。

### P7-7 动态变量（G7/G8）— `src/dynamic.rs` 内联单测（已落地）
- [x] `{{$uuid}}` → 合法 UUID v4 — `uuid_shape`
- [x] `{{$random.uuid}}` → 与 `$uuid` 等价 — `uuid_shape`
- [x] `{{$timestamp}}` → 10 位 unix 秒 — `timestamp_is_number`
- [x] `{{$isoTimestamp}}` → RFC3339（`2026-09-07T12:00:00Z` 形态）— `iso_timestamp_shape`
- [x] `{{$randomInt}}` / `{{$random.integer()}}` → `[0,1000)` 整数 — `random_integer_range`
- [x] `{{$random.integer(5,9)}}` → `[5,9]` 闭区间整数 — `random_integer_range`
- [x] `{{$historyFolder}}` → `<projectRoot>/.http-history`，目录被创建 — `history_folder_created`
- [x] `{{$projectRoot}}` → 绝对路径 — `history_folder_created`（断言以 project_root 为基）
- [x] 同一请求内两次 `{{$uuid}}` 产生**不同**值 — `uuid_shape`
- [x] 标识符含 `.` / `(` / `)` / `,` 时 `scan_env_var` 正确闭合 — `env.rs::dynamic_variable_scan`
- [x] 未闭合 `{{$uuid` 仍报错带位置 — `env.rs::substitute_undefined_preserved`（未定义/未闭合保留原文 + 警告）
- [x] `$jsonpath` 不当作动态变量 — `jsonpath_not_dynamic`；非 `$` 前缀不当作动态变量 — `non_dollar_not_dynamic`

### P7-8 变量作用域与 env 文件（G11/G21/G22）— `src/env.rs` 内联单测（已落地）
- [x] `load_env_files` 读 `http-client.env.json`（JetBrains `{"env":{"k":"v"}}` 格式）— `load_env_files_merges_private_and_shared`
- [x] `http-client.private.env.json` 覆盖同名键（私有优先）— `load_env_files_private_overrides_base`
- [x] 私有文件缺失时不报错 — `load_env_files_missing_base_errors_but_private_is_optional`、`private_sibling_naming`
- [x] `--env staging` 只取对应环境块 — `load_env_files_unknown_environment_errors` + `runner_test.rs`
- [x] 优先级：environment > global > file > request — `layer_precedence`
- [x] `VarValue::Json`：`request.variables.set("clients", [...])` 后 `get` 返回数组 — `handler_test.rs::glue_request_variables_set_captured_as_var_write`
- [x] JSON 值替换进文本：字符串直出、数字直出、对象/数组紧凑序列化 — `json_autoquote_string_value`、`json_number_bare`
- [x] 未定义变量保留 `{{name}}` 原文 + 警告诊断（回归）— `substitute_undefined_preserved`、`substitute_undefined_with_whitespace_preserved_verbatim`
- [x] 标识符含 `-`（`{{my-temp-variable}}`）可解析（回归）— `substitute_simple`、`substitute_with_internal_whitespace`

### P7-9 JSON 上下文自动引号（G24）— `src/env.rs` + `src/execute.rs` 内联单测（已落地）
- [x] `{"id": {{$random.uuid}}}` + `Content-Type: application/json` → `{"id": "…"}`（合法 JSON）— `execute.rs::prepare_json_body_autoquotes_string_dynamic_variable`
- [x] `{"id": "{{$random.uuid}}"}` → 不重复加引号 — `env.rs::json_string_value_inside_quotes_is_not_double_quoted`
- [x] `{"price": {{$random.integer()}}}` → 数字裸插，无引号 — `env.rs::json_number_bare`
- [x] `{"ts": {{$timestamp}}}` → 数字裸插 — `env.rs::json_number_bare`
- [x] 非 JSON Content-Type → 不加引号 — `env.rs::json_autoquote_string_value`（仅 JSON 上下文触发）
- [x] 字符串值内含 `"` / 换行 → 正确 JSON 转义 — `env.rs::json_autoquote_string_value`

### P7-10 JSONPath 与循环（G9/G10/G16）— `src/jsonpath.rs` + `src/execute.rs` 内联 + `runner_test.rs`（已落地）
- [x] `$.a.b` 单值 — `jsonpath.rs::child_path`
- [x] `$.arr[0]` 索引 — `jsonpath.rs::index_and_negative`
- [x] `$.clients..id` 递归下降 → `[1,2,3]` — `jsonpath.rs::recursive_descent_collects_all`、`descend_all`
- [x] `$.clients..firstName` → `["George","John","Eduardo"]` — `jsonpath.rs::recursive_descent_collects_all`
- [x] 路径不存在 → 空结果 + 警告诊断（不 panic）— `jsonpath.rs::no_match_empty`
- [x] `expand_iterations`：3 个多值模板 → 3 次迭代 — `execute.rs::expand_iterations_descendant_ids_produce_three_iterations`
- [x] 迭代 0 的 body 含 `"clientId": 1`，迭代 2 含 `"clientId": 3` — `execute.rs::expand_iterations_descendant_ids_produce_three_iterations`
- [x] 模板长度不一致 → 取最大值，短者取模循环 — `execute.rs::expand_iterations_multiple_templates_wrap_modulo`
- [x] 无 JSONPath 模板 → 恰好 1 次迭代 — `execute.rs::expand_iterations_no_jsonpath_template_is_single_empty`
- [x] `request.iteration()` 在 handler 中返回 0/1/2 — `handler_test.rs::glue_request_iteration_and_template_value`
- [x] `request.templateValue(0)` 返回当前迭代的 `clientId` — `handler_test.rs::glue_request_iteration_and_template_value`
- [x] httpmock 收到 3 次 POST，body 各不相同 — `runner_test.rs::loop_request_dispatches_once_per_jsonpath_match` + `example_files_test.rs::loop_example_runs_three_iterations_per_request`

### P7-11 x-www-form-urlencoded（G12）— `src/execute.rs` 内联单测（已落地）
- [x] 多行 `id = 999 &` ⏎ `value = content` → `id=999&value=content` — `format_urlencoded_multiline_pairs`
- [x] `%+` → `+`、`%=` → `=`、`%&` → `&`、`%%` → `%` — `format_urlencoded_escaped_ampersand_and_percent`
- [x] `fact = IntelliJ %+ HTTP Client %= <3` → `fact=IntelliJ+%2B+HTTP+Client+%3D+%3C3` — `format_urlencoded_escaped_ampersand_and_percent`
- [x] 空格编码为 `+`（form 规则），非 `%20` — `format_urlencoded_multiline_pairs`
- [x] key/value 两侧空白被 trim — `format_urlencoded_multiline_pairs`
- [x] 值内含 `&`（未转义）时按字面切分（与 JetBrains 一致）— `format_urlencoded_newline_separates_without_ampersand`
- [x] 非 urlencoded Content-Type 的 body 不受影响（回归）— `prepare_urlencoded_body_is_formatted`（仅 urlencoded 触发）
- [x] httpmock 断言收到的 `content-type` 与 body 字节 — `example_files_test.rs::post_example_runs_against_a_fake_httpbin`

### P7-12 Dispatch 差异化（G13）— `dispatch_test.rs` + `src/execute.rs` 内联（已落地）
- [x] 默认（无 tag）：3xx 被跟随（`Policy::limited`）— `redirect_followed_by_default_but_not_with_tag`
- [x] `@no-redirect`：301 原样返回，`Location` 头保留 — `redirect_followed_by_default_but_not_with_tag` + `example_files_test.rs::get_example_runs_against_a_fake_httpbin`（状态序列含 301）
- [x] `@no-cookie-jar`：`Set-Cookie` 不写入共享 jar — `no_cookie_jar_never_stores_or_replays_cookies`
- [x] 无 `@no-cookie-jar`：第二个请求自动带上第一个响应的 cookie — `cookies_shared_across_requests_in_jar`
- [ ] `@timeout 50` + 慢响应 → 超时错误诊断 —（**非 example 范围**：四个 example 均未使用 `@timeout`；tag 解析已由 `doc_tags_are_harvested_from_hash_comments` 锁定，离线超时用例从略）
- [x] `@no-auto-encoding`：非 ASCII target 原样上线 — `execute.rs::no_auto_encoding_tag_sends_target_verbatim`（`€uro` → 默认 `%E2%82%ACuro`、带 tag 原样）
- [x] 默认：非 ASCII query 被 percent-encode（回归）— `execute.rs::no_auto_encoding_tag_sends_target_verbatim`、`encode_path_basic`
- [x] `HTTP/2` 请求在 h2 mock/公网上协商成功 — `send_options_from_tags_and_http2_version` + `public_smoke_test.rs::smoke_http2_is_negotiated_over_tls`
- [x] client 按 opts 指纹复用（同 opts 两请求不重建 client）— `send_options_defaults_follow_redirects_and_cookie_jar`（SendOptions 指纹相等性）

### P7-13 JS 引擎 API（G14~G20）— `handler_test.rs` + `src/handler.rs` 内联（已落地）
- [x] `client.test("name", fn)` 通过 → `TestResult{passed:true}` — `glue_client_test_records_pass_and_fail`
- [x] `client.test` 内 `client.assert(false,"msg")` → `passed:false` + message — `glue_client_test_records_pass_and_fail`
- [x] `client.test` 内抛异常 → 捕获为失败，不中断后续 test — `glue_client_test_records_pass_and_fail`
- [x] 一个 handler 内多个 `client.test` 全部执行并各自记录 — `glue_client_test_records_pass_and_fail` + `example_files_test.rs`（Scripts 6 passed / 1 failed）
- [x] `client.assert(cond)` 无 message 时给出默认文案 — `glue_client_assert_outside_test_records_diagnostic`
- [x] 顶层 `assert(cond,msg)` 回归不破 — `handler_assert_pass_does_not_record_diagnostic`、`handler_assert_fail_records_diagnostic_continues_execution`
- [x] `client.global.clearAll()` 清空全部全局变量 — `glue_client_global_all_and_clear_all`
- [x] `client.global.clear("k")` 只清空指定键 — `glue_client_global_all_and_clear_all`
- [x] `client.global` 跨请求持久（headers 同理走 global 存储）— `handler_globals_persist_across_run_calls`、`runner_test.rs::globals_set_by_one_request_are_visible_to_the_next`
- [x] `client.exit()` 中断后续迭代 — `glue_client_exit_marks_runtime_without_error_diagnostic` + `runner_test.rs::client_exit_stops_remaining_requests`
- [x] `response.contentType.mimeType` == `application/json` — `glue_response_content_type_mimetype` + `handler.rs::content_type_parsing`
- [x] `response.contentType.charset` 解析 `; charset=utf-8` — `glue_response_content_type_mimetype` + `handler.rs::content_type_parsing`
- [x] `response.headers.valueOf("content-type")` 大小写不敏感 — `glue_response_headers_valueof_alias`、`handler_response_headers_value_case_insensitive`
- [x] `response.headers.valuesOf(name)` 返回数组 —（实现于 `handler.rs:789`；**非 example 范围**，无专用断言）
- [x] `response.body.hasOwnProperty("headers")` 可用（JSON 对象）— `handler_response_body_object_when_json` + Scripts example line 38 实跑
- [ ] `response.cookies()` 返回 cookie 列表 —（**未实现，非 example 范围**：四个 example 仅访问 `httpbin.org/cookies` URL，未调用该 JS API）
- [x] `request.method` / `request.url()` / `request.headers.valueOf()` — `glue_request_environment_and_body`
- [x] `request.body.tryGetSubstituted()` 返回替换后的 body 文本 — `glue_request_environment_and_body`
- [x] `request.environment.get("secret")` 读到私有 env 值 — `glue_request_environment_and_body`
- [x] `request.variables.set/get` 在前置脚本中生效并影响本次请求 — `glue_request_variables_set_captured_as_var_write` + `runner_test.rs::pre_request_script_variables_reach_the_wire`
- [x] `crypto.sha256().updateWithText("abc").digest().toHex()` == 已知向量 — `glue_crypto_sha256_chain_matches_vector` + `handler.rs::hash_and_hmac_vectors`
- [x] `crypto.hmac.sha256().withTextSecret("k").updateWithText("m").digest().toHex()` == 已知向量 — `glue_crypto_hmac_sha256_chain_matches_vector` + `handler.rs::hash_and_hmac_vectors`（RFC 4231）
- [x] `.toBase64()` 输出正确 —（实现于 `handler.rs:1091 __native.hexToBase64`；**非 example 范围**）
- [x] `sha512` / `sha1` / `md5` 各一条已知向量 — `handler.rs::hash_and_hmac_vectors`
- [x] 全局 `jsonPath(response.body, "$.json.balance")` 返回值 — `glue_jsonpath_extracts_nested_value`
- [x] `jsonPath` 多值 → JS 数组 — `glue_jsonpath_extracts_nested_value`
- [x] ESM：`import {makeSignature} from "./my-utils"` 在前置脚本中可用 — `glue_esm_import_resolves_and_calls_module_function` + `handler.rs::import_specifier_recognition`、`resolve_module_finds_js_sibling` + Scripts example 实跑
- [x] ESM：`import {findSignature} from "./my-utils"` 在响应处理器中可用 — `glue_esm_import_resolves_and_calls_module_function` + Scripts example 实跑
- [x] 省略 `.js` 扩展名可解析；模块文件缺失 → 诊断 — `handler.rs::resolve_module_finds_js_sibling`、`glue_esm_import_resolves_and_calls_module_function`
- [x] 脚本内 `{{var}}` 被替换后再求值 — `handler_test.rs` 各 glue 用例（脚本经 env 替换后执行）
- [x] ES6：`let`/`const`/箭头函数/模板字符串/解构 均可用 — 全部 `glue_*` 用例均以 ES6 书写
- [x] 脚本运行超时仍被中断（回归）— `handler_script_timeout_terminates_infinite_loop`

### P7-14 Runner 批量执行（G23）— `runner_test.rs`（11 测试全绿）
- [x] `run_file` 顺序执行文件内全部请求 — `runs_every_request_in_file_in_order`
- [x] `client.global` 跨请求持久（请求 A set → 请求 B `{{k}}` 可读）— `globals_set_by_one_request_are_visible_to_the_next`
- [x] `FileReport.tests_passed` / `tests_failed` 统计正确 — `failed_test_is_counted_and_later_requests_still_run`
- [x] 单请求失败不阻断后续请求 — 同上
- [x] 每个请求的 `>>` / `<>` 各自落盘 — `output_redirects_are_written_for_each_request`、`history_folder_redirect_is_rooted_at_the_base_dir`
- [x] 前置脚本 → 迭代展开 → dispatch → handler 的调用顺序正确 — `pre_request_script_variables_reach_the_wire`、`loop_request_dispatches_once_per_jsonpath_match`
- [x] `client.exit()` 中止后续请求 — `client_exit_stops_remaining_requests`
- [x] `RunOptions.request` 过滤（名称 / 1-based 序号）— `request_filter_selects_by_name_or_position`
- [x] `<>` handler 的相对路径按 `.http` 文件所在目录解析 — `run_file_resolves_handler_refs_next_to_the_http_file`
- [x] 循环变量的 JSON 值可来自 env 文件的 File 层 — `loop_reads_json_values_from_the_environment_file_layer`

### P7-15 CLI 接线 — `crates/http-cli/tests/cli_test.rs`（19 测试全绿）
- [x] `run file.http` 默认跑全文件（不再是首个请求）— `run_executes_every_request_in_the_file_by_default`
- [x] `--request <name|index>` 过滤仍生效（回归）— `run_with_request_name_selects_named_request` + `select_*` 单测
- [x] 输出含每条 test 的 `✓/✗` 与汇总行 — `print_report_human`（stdout 不便在进程内捕获，靠 example 实跑人工核对）
- [x] `tests_failed > 0` → 退出码 1；全通过 → 0 — `run_exits_non_zero_when_a_client_test_fails`
- [x] `--json` 输出可反序列化的 `FileReport` — `FileReport`/`IterationReport` 均 `#[derive(Serialize, Deserialize)]`
- [x] `--env dev` 加载 example 目录下的 env 文件 — `run_with_env_substitutes_variables` + `load_env_*` 单测

### P7-16 四个 example 端到端（离线）— `example_files_test.rs`（9 测试全绿）
- [x] `example/GET.http`：12 个请求全部解析成功、0 诊断错误
- [x] `example/POST.http`：4 个请求全部解析成功（含 multipart `< ./request-form-data.json`）
- [x] `example/RequestWithLoop.http`：2 个请求 → 分别展开 3 次迭代
- [x] `example/RequestWithScripts.http`：9 个请求全部解析成功（末尾裸 `###` 不产生请求）
- [x] 用 httpmock 替换 host 后逐请求执行，内嵌断言全部通过（Scripts 的"Failed test"用例**预期失败**：`tests_passed == 6 / tests_failed == 1` 且 `!report.ok()`）
- [x] fixtures 存在：`http-client.env.json`、`http-client.private.env.json`、`request-form-data.json`、`my-utils.js`
- [x] AST 关键字段用直接断言锁定（名称/标签/续行 query/`>>`+`>>!`/HTTP/2/multipart FileRef/前后置脚本计数）替代 insta 快照 —— 快照对 12+9 个请求噪声过大，改为可读断言：`get_example_keeps_names_tags_continuations_and_redirects`、`post_example_keeps_the_multipart_file_part`、`script_and_loop_examples_carry_their_pre_and_post_scripts`
- [x] 运行级：`get_example_runs_against_a_fake_httpbin`（状态序列 `[200×4, 301, 200×7]`，`@no-redirect` 生效，`.http-history/` 两个落盘文件）、`post_example_runs_against_a_fake_httpbin`、`loop_example_runs_three_iterations_per_request`（6 迭代 / 6 tests / 6 logs）

> 离线跑发现的真实缺口：JetBrains 惯例把 scheme 一并放进变量（`GET {{host}}/get`，`host = "https://httpbin.org"`）。
> 原实现先拼默认 `http://` 再替换，得到 `http://http://127.0.0.1:PORT/get`。
> 修复：`execute.rs::build_url` 在替换后对 authority 做 `split_once("://")`，前缀通过 `is_scheme`（RFC 3986）校验时内嵌 scheme 优先；
> 单测 `host_variable_may_carry_its_own_scheme` 覆盖「变量自带 scheme / 裸 host 用默认 scheme / 请求行显式 scheme 不被覆盖」三种情形。

### P7-17 公网 smoke（`#[ignore]`）— `public_smoke_test.rs`（8 测试，实连 httpbin.org 全绿，16.43s）
运行方式：`cargo test -p http-core --features js-handler --test public_smoke_test -- --ignored --test-threads 1`
- [x] `httpbin.org/get` 200 + `Content-Type: application/json` — `smoke_http2_is_negotiated_over_tls`
- [x] `httpbin.org/post` JSON → `.json` 字段回显 — `smoke_json_post_is_echoed`
- [x] `httpbin.org/post` form / multipart → 由 `smoke_post_example` 全文件覆盖（含 `< ./request-form-data.json`）
- [x] `httpbin.org/status/301` + `@no-redirect` → 301 — `smoke_no_redirect_tag_keeps_the_3xx`
- [x] `httpbin.org/cookies` + Cookie 头 → `.cookies` 回显 — `smoke_cookies_are_echoed_from_the_header`
- [x] `httpbin.org/get HTTP/2` → TLS+ALPN 协商为 h2 — `smoke_http2_is_negotiated_over_tls`
- [x] `run_file(example/GET.http)` 全绿（12 请求 + `.http-history` 落盘）— `smoke_get_example`
- [x] `run_file(example/POST.http)` 全绿 — `smoke_post_example`
- [x] `run_file(example/RequestWithLoop.http)` 全绿（6 次迭代 / 6 tests passed）— `smoke_loop_example`
- [x] `run_file(example/RequestWithScripts.http)`：6 passed / 1 failed（"Failed test"），`!report.ok()` — `smoke_scripts_example`

---

## 测试数据组织

```
crates/http-core/
├── src/                        # 细粒度特性的单测就地放在各模块的 #[cfg(test)] 中
│   ├── dynamic.rs              # P7-7 $uuid/$random.*/$timestamp/$isoTimestamp/$historyFolder
│   ├── env.rs                  # P7-8 env 文件分层 + P7-9 JSON 值自动加引号
│   ├── jsonpath.rs             # P7-10 JSONPath 求值
│   ├── execute.rs              # P7-6 host 变量自带 scheme、P7-10 迭代展开、P7-11 urlencoded
│   └── parser.rs / lexer.rs    # P7-1 doc tags、P7-3 前置脚本、P7-5 多行 handler、P7-12 `>>`
└── tests/
    ├── snapshots/                    # insta（由 handler_env_recovery_test 生成）
    ├── lexer_test.rs                 # P1-1~P1-2
    ├── parser_test.rs                # P1-3~P1-11
    ├── headers_body_test.rs          # P1-*
    ├── multipart_test.rs             # P1-*
    ├── execute_test.rs               # P2-1~P2-4
    ├── dispatch_test.rs              # P2-* / P7-9 SendOptions / P7-4 `>>` 落盘
    ├── handler_test.rs               # P3-1~P3-6 + P7-13（client.test/crypto/jsonPath/ESM/request.*）
    ├── handler_env_recovery_test.rs  # P3-*（insta 快照）
    ├── runner_test.rs                # P7-14
    ├── example_files_test.rs         # P7-16
    └── public_smoke_test.rs          # P7-17（全部 #[ignore]）

crates/http-cli/tests/cli_test.rs     # P7-15
example/                              # 四个 JetBrains 官方 example + fixtures（env/private.env/request-form-data.json/my-utils.js）
```

> 计划里原本设想为每个 P7 特性单开一个 `*_test.rs`；实施时改为「细粒度特性 → 模块内联单测，跨模块行为 → 既有集成测试文件」，
> 避免十余个只含两三个用例的小文件。测试数量与覆盖面不变（`cargo test --workspace --all-features` 全绿）。

## TDD 流程约定

1. **Red**：写测试，`cargo test` 失败
2. **Green**：最小实现让测试通过
3. **Refactor**：重构并保持绿色
4. **Commit**：每个绿 → 单独 commit（粒度：一个测试用例 + 其实现）
5. **快照**：insta 自动接受前必须人工 review diff
