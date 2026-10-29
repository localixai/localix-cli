import { fail, iso, validate, type State, type Task } from "./model";
export const dto = (t: Task) => {
  const { deps, history, comments, ...out } = t;
  return out;
};
export function make(
  b: any,
  p: any,
  u: number,
  s: State,
  field = (x: string) => x,
): Task {
  const title = typeof b.title === "string" ? b.title.trim() : "";
  validate(title.length > 0 && title.length <= 200, field("title"));
  const description = b.description === undefined ? "" : b.description;
  validate(typeof description === "string", field("description"));
  const priority = b.priority ?? "normal";
  validate(["low", "normal", "high"].includes(priority), field("priority"));
  const labels = b.labels ?? [];
  validate(
    Array.isArray(labels) &&
      labels.every(
        (x: any) =>
          typeof x === "string" &&
          x.trim().length <= 30 &&
          !/\s/.test(x.trim()),
      ),
    field("labels"),
  );
  const assignee = b.assigneeId ?? null;
  validate(
    assignee === null || [...p.members.keys()].includes(assignee),
    field("assigneeId"),
  );
  const n = ++p.seq,
    now = iso(s);
  return {
    id: ++s.counters.task,
    key: `${p.key}-${n}`,
    number: n,
    projectId: p.id,
    title,
    description,
    status: "open",
    priority,
    labels: [
      ...new Set(labels.map((x: string) => x.trim().toLowerCase())),
    ].sort(),
    assigneeId: assignee,
    createdBy: u,
    createdAt: now,
    updatedAt: now,
    version: 1,
    deps: new Set(),
    history: [],
    comments: [],
  };
}
export function etag(t: Task) {
  return { ETag: `"${t.version}"` };
}
export function checkMatch(req: Request, t: Task) {
  const h = req.headers.get("If-Match");
  if (h && h !== `"${t.version}"`) fail(412, "precondition_failed");
}
export function log(
  t: Task,
  u: number,
  s: State,
  field: string,
  from: any,
  to: any,
) {
  t.history.push({ at: iso(s), by: u, field, from, to });
}
