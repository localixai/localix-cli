import { type Ctx, auth, fail, noContent, ok, taskByKey } from "./http";
export function taskDelete(c: Ctx, path: string, method: string): Response | null {
  const m = path.match(/^\/tasks\/([^/]+)$/);
  if (!m || method !== "DELETE") return null;
  const key = m[1];
  if (!key) return null;
  const t = taskByKey(c, key), user = auth(c, "member", t.projectId);
  const role = c.s.members.get(t.projectId)!.get(user);
  if (role !== "owner" && t.createdBy !== user) fail(403, "forbidden", "Cannot delete this task");
  c.s.tasks = c.s.tasks.filter((x) => x !== t);
  for (const x of c.s.tasks) x.deps = x.deps.filter((key) => key !== t.key);
  return noContent();
}
