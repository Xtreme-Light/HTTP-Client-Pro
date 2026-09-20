/**
 * 内置请求示例 — 通过标题栏「请求示例」菜单以只读标签页打开。
 * 内容覆盖 .http 格式的完整用法（见项目 spec.md）。
 */

export interface ExampleEntry {
  id: string;
  /** 菜单/标签页显示的标题 */
  title: string;
  /** 完整的 .http 文件内容 */
  content: string;
}

const GET_EXAMPLE = `# ============================================================
# GET 请求示例
# ------------------------------------------------------------
# 以 ### 开头的块是一个独立请求，### 后的文字是请求名称，
# 会显示在 HISTORY 面板中。点击行号旁的绿色按钮即可发送。
# ============================================================

### 最简单的 GET 请求
GET https://httpbin.org/get

### 带查询参数的 GET 请求
# 查询参数直接写在 URL 上，多个参数用 & 连接
GET https://httpbin.org/get?page=1&size=10&keyword=http-client

### 带自定义请求头的 GET 请求
GET https://httpbin.org/headers
Accept: application/json
User-Agent: HTTP-Client-Pro
X-Request-Id: demo-0001

### 使用动态变量的 GET 请求
# {{$uuid}}/{{$timestamp}}/{{$randomInt}} 在每次运行时自动生成，
# 也可以用 {{$random.integer(1,6)}} 指定随机整数范围
GET https://httpbin.org/get?requestId={{$uuid}}&ts={{$timestamp}}&dice={{$random.integer(1,6)}}

### 不自动跟随重定向的 GET 请求
# 在请求前用注释行声明 # @no-redirect，返回原始 3xx 响应
# @no-redirect
GET https://httpbin.org/status/301

### 获取指定状态码（用于测试错误处理）
GET https://httpbin.org/status/404

### 获取 JSON / HTML / 图片等不同类型响应
GET https://httpbin.org/json

###
`;

const POST_EXAMPLE = `# ============================================================
# POST 请求示例
# ------------------------------------------------------------
# 请求头与请求体之间用一个空行分隔。
# ============================================================

### POST 发送 JSON 请求体
POST https://httpbin.org/post
Content-Type: application/json

{
  "id": 999,
  "value": "content",
  "tags": ["http", "client"]
}

### POST 发送 x-www-form-urlencoded 表单
# 键值对之间用 & 连接（行尾 & 可换行书写，运行时自动整理）
POST https://httpbin.org/post
Content-Type: application/x-www-form-urlencoded

id = 999 &
value = content &
fact = HTTP Client <3

### POST 请求体中使用动态变量
POST https://httpbin.org/post
Content-Type: application/json

{
  "id": "{{$uuid}}",
  "price": {{$randomInt}},
  "createdAt": {{$isoTimestamp}},
  "value": "content"
}

### POST 请求体引用本地文件
# 用 "< 文件路径" 将文件内容作为请求体（相对路径基于当前 .http 文件）
POST https://httpbin.org/post
Content-Type: application/json

< ./request-body.json

### 带认证头的 POST 请求
POST https://httpbin.org/post
Authorization: Bearer my-token
Content-Type: application/json

{
  "action": "create",
  "name": "demo"
}

###
`;

const FORM_EXAMPLE = `# ============================================================
# POST 表单（multipart/form-data）示例
# ------------------------------------------------------------
# Content-Type 中用 boundary= 声明分隔符，请求体中每个字段
# 以 --分隔符 开始；结束标记为 --分隔符--（两个减号结尾）。
# ============================================================

### 发送包含文本字段和文件字段的表单
POST https://httpbin.org/post
Content-Type: multipart/form-data; boundary=WebAppBoundary

--WebAppBoundary
Content-Disposition: form-data; name="element-name"
Content-Type: text/plain

Name
--WebAppBoundary
Content-Disposition: form-data; name="data"; filename="data.json"
Content-Type: application/json

< ./request-form-data.json
--WebAppBoundary--

### 发送多个文本字段的表单
POST https://httpbin.org/post
Content-Type: multipart/form-data; boundary=MyBoundary

--MyBoundary
Content-Disposition: form-data; name="username"

alice
--MyBoundary
Content-Disposition: form-data; name="email"

alice@example.com
--MyBoundary
Content-Disposition: form-data; name="bio"
Content-Type: text/plain

Hello, this is a multipart form field value.
--MyBoundary--

### 表单字段中使用动态变量
POST https://httpbin.org/post
Content-Type: multipart/form-data; boundary=MyBoundary

--MyBoundary
Content-Disposition: form-data; name="requestId"

{{$uuid}}
--MyBoundary
Content-Disposition: form-data; name="timestamp"

{{$timestamp}}
--MyBoundary--

###
`;

const ADVANCED_EXAMPLE = `# ============================================================
# 高级用法示例
# ------------------------------------------------------------
# 覆盖 PUT / PATCH / DELETE、环境变量、响应处理脚本、
# 响应输出重定向与常用文档标签。
# ============================================================

### PUT 更新资源
PUT https://httpbin.org/put
Content-Type: application/json

{
  "id": 1,
  "name": "updated-name"
}

### PATCH 局部更新资源
PATCH https://httpbin.org/patch
Content-Type: application/json

{
  "name": "patched-name"
}

### DELETE 删除资源
DELETE https://httpbin.org/delete

### 使用环境变量 {{变量名}}
# 变量来自当前选中的环境（左侧栏 ENVIRONMENTS 面板管理），
# 可用于 URL、请求头、请求体的任意位置
GET {{host}}/get?token={{token}}
Authorization: Bearer {{token}}

### 响应处理脚本：断言与提取变量
# 请求后跟 "> {%" 与 "%}" 之间的 JavaScript，在响应返回后执行；
# client.global.set 写入的变量可被后续请求用 {{变量名}} 引用
GET https://httpbin.org/get

> {%
    client.test("Request executed successfully", function () {
        client.assert(response.status === 200, "Response status is not 200");
    });

    client.test("Response is JSON", function () {
        client.assert(response.contentType.mimeType === "application/json",
            "Response is not JSON");
    });

    // 提取响应字段为全局变量，供后续请求使用
    client.global.set("origin", response.body.origin);
%}

### 引用上一个请求提取的变量
GET https://httpbin.org/get?origin={{origin}}

### 将响应输出重定向到文件
# ">> 路径" 追加保存（重名自动加后缀），">>! 路径" 强制覆盖；
# {{$historyFolder}} 指向项目的 .http-history 目录
GET https://httpbin.org/get
>>! {{$historyFolder}}/latest-response.json

### 请求前脚本：预计算变量
# "< {%" 与 "%}" 之间的 JavaScript 在请求发送前执行
< {%
    request.variables.set("signature", "sig-" + Date.now());
%}
POST https://httpbin.org/post
X-Signature: {{signature}}
Content-Type: application/json

{
  "prop": "value"
}

### 常用文档标签一览
# # @no-redirect        不自动跟随 3xx 重定向
# # @no-cookie-jar      不发送也不保存 Cookie
# # @no-auto-encoding   路径/查询参数原样发送，不做百分号编码
# # @no-log             不记录该请求的执行历史
# # @name 请求名        显式指定请求名（优先于 ### 后的文字）
# @no-cookie-jar
GET https://httpbin.org/cookies

###
`;

export const EXAMPLES: ExampleEntry[] = [
  { id: 'get', title: 'GET 请求示例', content: GET_EXAMPLE },
  { id: 'post', title: 'POST 请求示例', content: POST_EXAMPLE },
  { id: 'form', title: 'POST 表单 (multipart/form-data) 示例', content: FORM_EXAMPLE },
  { id: 'advanced', title: '高级用法示例', content: ADVANCED_EXAMPLE },
];

export function getExample(id: string): ExampleEntry | undefined {
  return EXAMPLES.find((e) => e.id === id);
}
