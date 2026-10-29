import { err, iso, ok, publicTask, type Project, type State, type Task } from "./model";
const key = (p: number, u: number) => `${p}:${u}`;
const role = (s: State, p: Project, u: number) =>
  u === p.ownerId ? "owner" : s.memberships.get(key(p.id, u));
const member = (s: State, p: Project, u: number) => !!role(s, p, u);
export function validateTask(b: any, field = "") {
  const pref = field ? field + "." : "";
  if (typeof b.title !== "string" || !b.title.trim() || b.title.length > 200) return pref + "title";
  if (b.description !== undefined && typeof b.description !== "string") return pref + "description";
  if (b.priority !== undefined && !["low", "normal", "high"].includes(b.priority))
    return pref + "priority";
  if (
    b.labels !== undefined &&
    (!Array.isArray(b.labels) ||
      b.labels.some(
        (x: any) =>
          typeof x !== "string" || !x.trim() || x.trim().length > 30 || /\s/.test(x.trim()),
      ))
  )
    return pref + "labels";
  return "";
}
export function makeTask(s: State, p: Project, u: number, b: any): Task {
  const id = s.tasks.length + 1,
    number = p.next++,
    at = iso(s),
    labels = [
      ...new Set<string>((b.labels ?? []).map((x: string) => x.trim().toLowerCase())),
    ].sort();
  return {
    id,
    number,
    projectId: p.id,
    title: b.title.trim(),
    description: b.description ?? "",
    status: "open",
    priority: b.priority ?? "normal",
    labels,
    assigneeId: b.assigneeId ?? null,
    createdBy: u,
    createdAt: at,
    updatedAt: at,
    version: 1,
    deleted: false,
    deps: [],
    history: [],
  };
}
export function createTasks(
  s: State,
  p: Project,
  b: any,
  u: number | undefined,
  token: string,
  idem: string,
): Response {
  if (!u) return err("unauthorized", "unauthorized", 401);
  if (!member(s, p, u)) return err("not_found", "not found", 404);
  if (role(s, p, u) === "viewer") return err("forbidden", "forbidden", 403);
  const idemKey = idem ? `${u}:${p.id}:${idem}` : "";
  const prior = idemKey ? s.idem.get(idemKey) : undefined;
  const signature = JSON.stringify(b);
  if (prior && s.now() - prior.at < 86400000) {
    if (prior.signature !== signature)
      return err("idempotency_mismatch", "idempotency key reused", 422);
    const replay = Response.json(prior.body, { status: prior.status, headers: prior.headers });
    replay.headers.set("Idempotent-Replayed", "true");
    return replay;
  }
  const bulk = Array.isArray(b.tasks);
  const arr = bulk ? b.tasks : [b];
  if (bulk && (!arr.length || arr.length > 50))
    return err("validation", "invalid task list", 400, "tasks");
  for (let i = 0; i < arr.length; i++) {
    const field = validateTask(arr[i], bulk ? `tasks[${i}]` : "");
    if (field) return err("validation", "invalid task", 400, field);
    if (arr[i].assigneeId != null && !member(s, p, arr[i].assigneeId))
      return err(
        "validation",
        "invalid assignee",
        400,
        bulk ? `tasks[${i}].assigneeId` : "assigneeId",
      );
  }
  const items = arr.map((x: any) => {
    const t = makeTask(s, p, u, x);
    s.tasks.push(t);
    return publicTask(t, p);
  });
  const response = ok(bulk ? { items } : { ...items[0] }, 201, { ETag: '"1"' });
  if (idemKey)
    s.idem.set(idemKey, {
      at: s.now(),
      signature,
      status: 201,
      body: bulk ? { items } : items[0],
      headers: { ETag: '"1"' },
    });
  return response;
}
export function listTasks(s: State, p: Project, url: URL, u: number | undefined): Response {
  if (!u) return err("unauthorized", "unauthorized", 401);
  if (!member(s, p, u)) return err("not_found", "not found", 404);
  const q = url.searchParams,
    validStatus = ["open", "in_progress", "done"];
  let ts = s.tasks.filter((t) => t.projectId === p.id && !t.deleted);
  if (q.has("status")) {
    if (!validStatus.includes(q.get("status")!))
      return err("validation", "invalid status", 400, "status");
    ts = ts.filter((t) => t.status === q.get("status"));
  }
  if (q.has("assignee")) {
    const v = q.get("assignee")!;
    if (v === "me") ts = ts.filter((t) => t.assigneeId === u);
    else if (v === "none") ts = ts.filter((t) => t.assigneeId === null);
    else if (/^\d+$/.test(v)) ts = ts.filter((t) => t.assigneeId === Number(v));
    else return err("validation", "invalid assignee", 400, "assignee");
  }
  if (q.has("label")) ts = ts.filter((t) => t.labels.includes(q.get("label")!.toLowerCase()));
  if (q.has("q")) {
    const v = q.get("q")!.toLowerCase();
    ts = ts.filter((t) => (t.title + " " + t.description).toLowerCase().includes(v));
  }
  const sort = q.get("sort") ?? "created";
  if (!["created", "-created", "priority"].includes(sort))
    return err("validation", "invalid sort", 400, "sort");
  if (sort === "-created") ts.reverse();
  if (sort === "priority") {
    const n = { high: 0, normal: 1, low: 2 };
    ts.sort(
      (a, b) => n[a.priority as keyof typeof n] - n[b.priority as keyof typeof n] || a.id - b.id,
    );
  }
  const limitText = q.get("limit") ?? "20",
    limit = Number(limitText);
  if (!/^\d+$/.test(limitText) || limit < 1 || limit > 100)
    return err("validation", "invalid limit", 400, "limit");
  let offset = 0;
  const cursor = q.get("cursor");
  if (cursor) {
    try {
      offset = Number(atob(cursor));
      if (!Number.isInteger(offset) || offset < 0) throw 0;
    } catch {
      return err("validation", "invalid cursor", 400, "cursor");
    }
  }
  const page = ts.slice(offset, offset + limit);
  const next = offset + limit < ts.length ? btoa(String(offset + limit)) : null;
  return ok({ items: page.map((t) => publicTask(t, p)), nextCursor: next });
}
