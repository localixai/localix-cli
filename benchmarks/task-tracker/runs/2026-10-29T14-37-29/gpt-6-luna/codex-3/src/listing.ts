import { State, Project, err, ok } from "./model";
export function listTasks(s: State, r: Request, p: Project, u: number) {
  const q = new URL(r.url).searchParams;
  let ts = s.tasks.filter((t) => t.projectId === p.id);
  const status = q.get("status"),
    ass = q.get("assignee"),
    label = q.get("label"),
    search = q.get("q")?.toLowerCase();
  if (status && !["open", "in_progress", "done"].includes(status))
    return err("validation", "Invalid status", "status");
  if (status) ts = ts.filter((t) => t.status === status);
  if (ass) {
    if (!["me", "none"].includes(ass) && !/^\d+$/.test(ass))
      return err("validation", "Invalid assignee", "assignee");
    ts = ts.filter((t) =>
      ass === "me"
        ? t.assigneeId === u
        : ass === "none"
          ? t.assigneeId === null
          : t.assigneeId === +ass,
    );
  }
  if (label) ts = ts.filter((t) => t.labels.includes(label.toLowerCase()));
  if (search)
    ts = ts.filter((t) =>
      (t.title + " " + t.description).toLowerCase().includes(search),
    );
  const sort = q.get("sort") || "created";
  if (!["created", "-created", "priority"].includes(sort))
    return err("validation", "Invalid sort", "sort");
  ts.sort(
    sort === "-created"
      ? (a, b) => b.number - a.number
      : sort === "priority"
        ? (a, b) =>
            ["high", "normal", "low"].indexOf(a.priority) -
              ["high", "normal", "low"].indexOf(b.priority) ||
            a.number - b.number
        : (a, b) => a.number - b.number,
  );
  let n = 20;
  const raw = q.get("limit");
  if (raw) {
    n = Number(raw);
    if (!Number.isInteger(n) || n < 1 || n > 100)
      return err("validation", "Invalid limit", "limit");
  }
  let start = 0;
  const cursor = q.get("cursor");
  if (cursor) {
    try {
      const c = JSON.parse(atob(cursor));
      if (
        c.sort !== sort ||
        c.n !== n ||
        c.filters !== JSON.stringify([status, ass, label, search])
      )
        throw 0;
      start = ts.findIndex((t) => t.key === c.last) + 1;
      if (!start) throw 0;
    } catch {
      return err("validation", "Invalid cursor", "cursor");
    }
  }
  const items = ts.slice(start, start + n),
    more = start + n < ts.length;
  return ok({
    items: items.map((t) => { const { deps, history, comments, ...view } = t; return view; }),
    nextCursor: more
      ? btoa(
          JSON.stringify({
            sort,
            n,
            filters: JSON.stringify([status, ass, label, search]),
            last: items.at(-1)!.key,
          }),
        )
      : null,
  });
}
