import type { RequestCtx } from "./http";
import { denied, missing } from "./http";
import { memberRole, taskByKey } from "./model";

export function access(c: RequestCtx, key: string, write = false) {
  const t = taskByKey(c.state, key);
  if (!t || !memberRole(c.state, t.projectId, c.userId)) return { result: missing() };
  const role = memberRole(c.state, t.projectId, c.userId);
  if (write && role === "viewer") return { result: denied() };
  return { task: t, role };
}
export function versionCheck(c: RequestCtx, version: number) {
  const match = c.req.headers.get("If-Match");
  return match !== null && match !== `"${version}"`;
}
export function etag(version: number) { return { ETag: `"${version}"` }; }
export function normalizeLabels(v: unknown): string[] | null {
  if (!Array.isArray(v) || v.some(x => typeof x !== "string")) return null;
  const a = v.map(x => x.trim().toLowerCase());
  if (a.some(x => !x || x.length > 30 || /\s/.test(x))) return null;
  return [...new Set(a)].sort();
}
