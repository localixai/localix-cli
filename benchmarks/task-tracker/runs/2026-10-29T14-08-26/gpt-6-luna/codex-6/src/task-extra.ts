import { State, Task, Project, stamp, taskView } from "./state";
import { answer, auth, fail, writeRole, precondition, etag } from "./http";
export function transition(
  s: State,
  r: Request,
  t: Task,
  p: Project,
  user: number,
  b: any,
  now: () => number,
) {
  writeRole(s, p, user);
  precondition(r, t);
  if (!["open", "in_progress", "done"].includes(b.to))
    fail(400, "validation", "Invalid status", "to");
  const valid =
    (t.status === "open" && b.to === "in_progress") ||
    (t.status === "in_progress" && ["open", "done"].includes(b.to)) ||
    (t.status === "done" && b.to === "open");
  if (!valid) fail(409, "invalid_transition", "Invalid transition");
  if (
    b.to === "done" &&
    [...t.deps].some((k) =>
      s.tasks.some((x) => x.key === k && x.status !== "done"),
    )
  )
    fail(409, "blocked", "Blocked");
  const from = t.status,
    at = stamp(now);
  t.status = b.to;
  t.version++;
  t.updatedAt = at;
  s.history.get(t.key)!.push({ at, by: user, field: "status", from, to: b.to });
  return answer(taskView(t), 200, etag(t));
}
export function comments(
  s: State,
  r: Request,
  t: Task,
  user: number,
  m: string,
  b: any,
  now: () => number,
) {
  if (m === "GET")
    return answer(
      s.comments.filter((c) => c.key === t.key).map(({ key, ...c }) => c),
    );
  if (typeof b.body !== "string" || !b.body.trim() || b.body.length > 2000)
    fail(400, "validation", "Invalid body", "body");
  const c = {
    id: s.nextComment++,
    authorId: user,
    body: b.body.trim(),
    createdAt: stamp(now),
    key: t.key,
  };
  s.comments.push(c);
  const { key, ...out } = c;
  return answer(out, 201);
}
export function dependencies(
  s: State,
  r: Request,
  t: Task,
  m: string,
  a: string[],
  b: any,
  user: number,
) {
  if (m === "GET")
    return answer({
      key: t.key,
      dependsOn: [...t.deps].sort(
        (x, y) => Number(x.split("-").pop()) - Number(y.split("-").pop()),
      ),
      blocks: s.tasks
        .filter((x) => x.deps.has(t.key))
        .map((x) => x.key)
        .sort(
          (x, y) => Number(x.split("-").pop()) - Number(y.split("-").pop()),
        ),
    });
  const p = s.projects.find((x) => x.id === t.projectId)!;
  writeRole(s, p, user);
  if (m === "DELETE") {
    const dep = a[3];
    if (!t.deps.has(dep!)) fail(404, "not_found", "Not found");
    t.deps.delete(dep!);
    return answer(null, 204);
  }
  const target = s.tasks.find((x) => x.key === b.dependsOn);
  if (!target || target.projectId !== t.projectId || target === t)
    fail(400, "validation", "Invalid dependency", "dependsOn");
  if (t.deps.has(target!.key)) fail(409, "conflict", "Dependency exists");
  const reaches = (
    from: Task,
    goal: string,
    seen = new Set<string>(),
  ): boolean => {
    if (from.key === goal) return true;
    if (seen.has(from.key)) return false;
    seen.add(from.key);
    return [...from.deps].some((k) => {
      const x = s.tasks.find((y) => y.key === k);
      return !!x && reaches(x, goal, seen);
    });
  };
  if (reaches(target!, t.key))
    fail(409, "dependency_cycle", "Dependency cycle");
  t.deps.add(target!.key);
  return answer({ key: t.key, dependsOn: [...t.deps].sort(), blocks: [] }, 201);
}
