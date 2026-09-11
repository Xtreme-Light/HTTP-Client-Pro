// Shared helpers imported by RequestWithScripts.http via `import { … } from "./my-utils"`.
// The HTTP Client evaluates ES modules in the same sandbox as the handler script,
// so the `crypto` global is available here.

export function makeSignature() {
  return crypto
    .sha256()
    .updateWithText("payload-to-sign")
    .digest()
    .toHex();
}

// httpbin echoes request headers under `response.body.headers`; header names are
// matched case-insensitively so this works regardless of canonicalisation.
export function findSignature(body) {
  const headers = body && body.headers ? body.headers : {};
  for (const key in headers) {
    if (key.toLowerCase() === "x-my-signature") {
      return headers[key];
    }
  }
  return undefined;
}
