/**
 * 将 Windows 反斜杠路径统一为正斜杠，保证跨平台下路径字符串可比较。
 *
 * 后端（Rust）在 Windows 上返回的路径使用 `\` 分隔，而前端引导逻辑用 `/`
 * 拼接，导致同一文件产生两个不同源的字符串。统一规范化为正斜杠后：
 * - 标签页去重（按路径精确匹配）生效；
 * - 侧栏 `parentDirOf` / 新建 / 重命名等基于 `/` 的拼接一致；
 * - Rust `std::fs` 在 Windows 上同样接受正斜杠，读写不受影响。
 */
export function normalizePath(p: string): string {
  return p.replace(/\\/g, '/');
}
