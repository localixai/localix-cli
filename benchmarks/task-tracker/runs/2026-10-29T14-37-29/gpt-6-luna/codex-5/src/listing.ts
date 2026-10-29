import { json, validation, type Ctx, type Task } from "./model";
const priorities = ["low", "normal", "high"];
const output = (t: Task) => {
  const { deleted, depends, comments, history, ...out } = t;
  return out;
};
export function list(c: Ctx, pid: number) {
  const q = c.url.searchParams,
    valid = (f: string) => validation(f);
  let a = c.db.tasks.filter((t) => t.projectId === pid && !t.deleted);
  const s = q.get("status");
  if (s) {
    if (!["open", "in_progress", "done"].includes(s)) return valid("status");
    a = a.filter((t) => t.status === s);
  }
  const as = q.get("assignee");
  if (as) {
    if (as === "none") a = a.filter((t) => t.assigneeId === null);
    else if (as === "me") a = a.filter((t) => t.assigneeId === c.user!.id);
    else if (/^\d+$/.test(as)) a = a.filter((t) => t.assigneeId === Number(as));
    else return valid("assignee");
  }
  const label = q.get("label");
  if (label) a = a.filter((t) => t.labels.includes(label.toLowerCase()));
  const search = q.get("q")?.toLowerCase();
  if (search)
    a = a.filter((t) =>
      (t.title + t.description).toLowerCase().includes(search),
    );
  const sort = q.get("sort") || "created";
  if (!["created", "-created", "priority"].includes(sort)) return valid("sort");
  a.sort((x, y) =>
    sort === "-created"
      ? y.number - x.number
      : sort === "priority"
        ? priorities.indexOf(y.priority) - priorities.indexOf(x.priority) ||
          x.number - y.number
        : x.number - y.number,
  );
  const lim = q.get("limit") || "20";
  if (!/^\d+$/.test(lim) || Number(lim) < 1 || Number(lim) > 100)
    return valid("limit");
  let start = 0;
  const cursor = q.get("cursor");
  if (cursor) {
    try {
      const cdata = JSON.parse(atob(cursor));
      if (cdata.sort !== sort) throw 0;
      start = a.findIndex((t) => t.key === cdata.last) + 1;
      if (!start) throw 0;
    } catch {
      return valid("cursor");
    }
  }
  const items = a.slice(start, start + Number(lim));
  const more = start + items.length < a.length;
  return json({
    items: items.map(output),
    nextCursor: more
      ? btoa(JSON.stringify({ sort, last: items.at(-1)!.key }))
      : null,
  });
}
