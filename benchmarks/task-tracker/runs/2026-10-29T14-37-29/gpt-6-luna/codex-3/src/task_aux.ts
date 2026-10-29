import { State, Task, err, ok, iso } from "./model";
export async function dependencies(
  s: State,
  r: Request,
  m: string,
  t: Task,
  u: number,
  role: string,
) {
  if (m === "GET") {
    const by = (k: string) =>
      s.tasks
        .filter((x) => x.projectId === t.projectId && x.deps.has(t.key))
        .map((x) => x.key);
    return ok({
      key: t.key,
      dependsOn: [...t.deps].sort(
        (a, b) => Number(a.split("-").at(-1)) - Number(b.split("-").at(-1)),
      ),
      blocks: by(t.key),
    });
  }
  if (m === "POST") {
    if (role === "viewer") return err("forbidden", "Forbidden", undefined, 403);
    let x: any;
    try {
      x = await r.json();
    } catch {
      return err("validation", "Invalid body", "body");
    }
    const d = byKey(s, x?.dependsOn);
    if (!d || d === t || d.projectId !== t.projectId)
      return err("validation", "Invalid dependency", "dependsOn");
    if (t.deps.has(d.key))
      return err("conflict", "Already exists", undefined, 409);
    const reaches = (cur: Task, seen = new Set<string>()): boolean =>
      cur.deps.has(t.key) ||
      [...cur.deps].some((k) => {
        if (seen.has(k)) return false;
        seen.add(k);
        const z = byKey(s, k);
        return z ? reaches(z, seen) : false;
      });
    if (reaches(d))
      return err("dependency_cycle", "Dependency cycle", undefined, 409);
    t.deps.add(d.key);
    return ok({ key: t.key, dependsOn: [...t.deps].sort(), blocks: [] }, 201);
  }
  const key = decodeURIComponent(r.url.split("/").at(-1)!);
  if (m === "DELETE") {
    if (role === "viewer") return err("forbidden", "Forbidden", undefined, 403);
    if (!t.deps.delete(key))
      return err("not_found", "Not found", undefined, 404);
    return new Response(null, { status: 204 });
  }
  return err("method_not_allowed", "Method not allowed", undefined, 405);
}
export function byKey(s: State, key: string) { return s.tasks.find((t) => t.key === key); }
export async function comments(s: State, r: Request, m: string, t: Task, u: number) {
  if (m === "GET") return ok(t.comments);
  if (m === "POST") {
    let x: any;
    try {
      x = await r.json();
    } catch {
      return err("validation", "Invalid body", "body");
    }
    if (typeof x?.body !== "string" || !x.body.trim() || x.body.length > 2000)
      return err("validation", "Invalid comment", "body");
    const c = {
      id: ++s.ids.comment,
      authorId: u,
      body: x.body.trim(),
      createdAt: iso(s),
    };
    t.comments.push(c);
    return ok(c, 201);
  }
  return undefined;
}

export function taskView(t: Task) {
 const { deps, history, comments, ...view } = t;
 return view;
}
