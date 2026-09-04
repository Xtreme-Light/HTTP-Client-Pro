import { describe, it, expect } from 'vitest';
import { looksLikeCurl, curlToHttp, shellSplit, composeCurlPasteInsertion } from './curl-to-http';

const EXAMPLE_CURL = [
  'curl --url `https://ug.baidu.com/mcp/pc/pcsearch` \\',
  "  -H 'Accept: */*' \\",
  "  -H 'Accept-Language: zh-CN,zh;q=0.5' \\",
  "  -H 'Cache-Control: no-cache' \\",
  "  -H 'Connection: keep-alive' \\",
  "  -H 'Content-Type: application/json' \\",
  "  -b 'H_WISE_SIDS=63147_65592; BAIDUID=8ADDAD45ACC86E0C37E4CA4627D3B520:FG=1' \\",
  "  -H 'Origin: https://www.baidu.com' \\",
  "  -H 'User-Agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/152.0.0.0' \\",
  "  -H 'sec-ch-ua: \"Chromium\";v=\"152\", \"Brave\";v=\"152\"' \\",
  `  --data-raw '{"invoke_info":{"pos_1":[{}],"pos_2":[{}],"pos_3":[{}]}}'`,
].join('\n');

describe('looksLikeCurl', () => {
  it('识别 curl 命令', () => {
    expect(looksLikeCurl(EXAMPLE_CURL)).toBe(true);
    expect(looksLikeCurl("curl 'https://example.com/api'")).toBe(true);
    expect(looksLikeCurl('  curl -X GET http://example.com')).toBe(true);
  });

  it('不误判普通文本', () => {
    expect(looksLikeCurl('GET http://example.com')).toBe(false);
    expect(looksLikeCurl('# curl 使用说明')).toBe(false);
    expect(looksLikeCurl('curling iron')).toBe(false);
    expect(looksLikeCurl('')).toBe(false);
  });
});

describe('shellSplit', () => {
  it('处理引号与转义', () => {
    expect(shellSplit(`a 'b c' "d \\"e" f\\ g`)).toEqual(['a', 'b c', 'd "e', 'f g']);
    expect(shellSplit(`curl -H 'a: "b"' x`)).toEqual(['curl', '-H', 'a: "b"', 'x']);
  });
});

describe('curlToHttp', () => {
  it('转换需求示例：头部注释 + POST + 请求头 + Cookie + body', () => {
    const out = curlToHttp(EXAMPLE_CURL);
    const lines = out.split('\n');

    // 原始 curl 每行转为 # 注释
    expect(lines[0]).toBe('# curl --url `https://ug.baidu.com/mcp/pc/pcsearch` \\');
    expect(lines[1]).toBe("#   -H 'Accept: */*' \\");

    // 注释与请求之间有空行
    const blankIdx = lines.findIndex((l) => l === '');
    expect(blankIdx).toBeGreaterThan(1);
    // 有 body 时 header 与 body 之间有空行（spec §3.2）
    expect(lines[lines.length - 2]).toBe('');

    // 无 -X 且有 --data-raw → POST；URL 去除了反引号
    expect(lines[blankIdx + 1]).toBe('POST https://ug.baidu.com/mcp/pc/pcsearch');

    expect(out).toContain('Accept: */*');
    expect(out).toContain('Content-Type: application/json');
    expect(out).toContain('sec-ch-ua: "Chromium";v="152", "Brave";v="152"');
    expect(out).toContain('Cookie: H_WISE_SIDS=63147_65592; BAIDUID=8ADDAD45ACC86E0C37E4CA4627D3B520:FG=1');
    expect(out.endsWith('{"invoke_info":{"pos_1":[{}],"pos_2":[{}],"pos_3":[{}]}}')).toBe(true);
  });

  it('无 body 时默认 GET，无 Cookie/空 body', () => {
    const out = curlToHttp("curl 'https://example.com/api?a=1'");
    const lines = out.split('\n');
    const blankIdx = lines.findIndex((l) => l === '');
    expect(lines[blankIdx + 1]).toBe('GET https://example.com/api?a=1');
    expect(lines.slice(blankIdx + 1)).toHaveLength(1);
  });

  it('显式 -X 优先，-H 原样保留', () => {
    const out = curlToHttp(`curl -X DELETE https://example.com/x -H 'X-Token: abc'`);
    expect(out).toContain('DELETE https://example.com/x');
    expect(out).toContain('X-Token: abc');
  });

  it('多个 -d 以 & 连接（curl 语义）', () => {
    const out = curlToHttp(`curl https://example.com -d a=1 -d b=2`);
    expect(out).toContain('POST https://example.com');
    expect(out.endsWith('a=1&b=2')).toBe(true);
  });

  it('-u 转为 Basic Authorization', () => {
    const out = curlToHttp(`curl -u user:pass https://example.com`);
    expect(out).toContain(`Authorization: Basic ${btoa('user:pass')}`);
  });

  it('无法提取 URL 时仅保留注释', () => {
    const out = curlToHttp('curl -s -v');
    expect(out).toBe('# curl -s -v');
  });

  it('URL 引号内混入换行（折行粘贴）时请求行仍为单行', () => {
    // 回归：粘贴来源的 curl 常在 --url 值内夹带换行，
    // 曾导致 "POST" 与 URL 被拆成两行
    const out = curlToHttp(`curl --url '\`https://ug.baidu.com/mcp/pc/pcsearch\`\n' \\\n  -H 'Accept: */*' \\\n  --data-raw '{"a":1}'`);
    const lines = out.split('\n');
    const reqIdx = lines.findIndex((l) => l === '') + 1;
    expect(lines[reqIdx]).toBe('POST https://ug.baidu.com/mcp/pc/pcsearch');
    expect(lines[reqIdx + 1]).toBe('Accept: */*');
  });

  it('URL 开始引号后紧跟换行时仍能提取', () => {
    const out = curlToHttp(`curl --url '\nhttps://example.com/api'`);
    const lines = out.split('\n');
    const reqIdx = lines.findIndex((l) => l === '') + 1;
    expect(lines[reqIdx]).toBe('GET https://example.com/api');
  });

  it('header 值内混入换行时压平为单行', () => {
    const out = curlToHttp(`curl https://example.com -H 'X-A:\n1'`);
    expect(out).toContain('X-A:1');
    expect(out.split('\n').some((l) => l === 'X-A:')).toBe(false);
  });
});

describe('composeCurlPasteInsertion', () => {
  const block = '# curl x\n\nGET http://x';

  it('空文档：直接插入块', () => {
    expect(composeCurlPasteInsertion('', '', block)).toBe(block);
  });

  it('已有内容（行首）：自动补 ### 分隔符', () => {
    expect(composeCurlPasteInsertion('GET http://a\n', '', block)).toBe('###\n' + block);
  });

  it('已有内容（行中）：先换行再补分隔符', () => {
    expect(composeCurlPasteInsertion('GET http://a', '', block)).toBe('\n###\n' + block);
  });

  it('光标紧跟 ### 分隔行之后：附加到该分隔符，不重复添加', () => {
    expect(composeCurlPasteInsertion('### 名称\n', '', block)).toBe(block);
    expect(composeCurlPasteInsertion('###\n', '', block)).toBe(block);
  });

  it('光标在 ### 行尾（无换行）：换行后附加到该分隔符', () => {
    expect(composeCurlPasteInsertion('###', '', block)).toBe('\n' + block);
  });

  it('后面有内容时补换行', () => {
    expect(composeCurlPasteInsertion('', '### Next\nGET http://b', block)).toBe(block + '\n');
  });
});
