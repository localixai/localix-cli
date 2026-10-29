import { publicTask } from "./taskModel";

const fail = (field: string) => new Response(JSON.stringify({
  error: { code: "validation", message: "validation", field },
}), { status: 400, headers: { "Content-Type": "application/json" } });

export function listing(records: any[], url: URL, userId: number): Response {
  const status = url.searchParams.get("status");
  const sort = url.searchParams.get("sort");
  const limitText = url.searchParams.get("limit");
  if (status && !["open", "in_progress", "done"].includes(status)) return fail("status");
  if (sort && !["created", "-created", "priority"].includes(sort)) return fail("sort");
  if (limitText && (!/^\d+$/.test(limitText) || Number(limitText) < 1 || Number(limitText) > 100))
    return fail("limit");
  if (url.searchParams.has("cursor") && url.searchParams.get("cursor") !== "0") return fail("cursor");
  const assignee = url.searchParams.get("assignee");
  if (assignee && !["me", "none"].includes(assignee) && !/^\d+$/.test(assignee))
    return fail("assignee");
  let list = records.filter((t) => !status || t.status === status);
  if (assignee) list = list.filter((t) => assignee === "none" ? !t.assigneeId :
    t.assigneeId === (assignee === "me" ? userId : Number(assignee)));
  const label = url.searchParams.get("label");
  if (label) list = list.filter((t) => t.labels.includes(label.toLowerCase()));
  const q = url.searchParams.get("q")?.toLowerCase();
  if (q) list = list.filter((t) => `${t.title} ${t.description}`.toLowerCase().includes(q));
  if (sort === "-created") list.reverse();
  if (sort === "priority") list.sort((a, b) =>
    ["high", "normal", "low"].indexOf(a.priority) - ["high", "normal", "low"].indexOf(b.priority));
  const limit = Number(limitText ?? 20);
  const offset = url.searchParams.has("cursor") ? limit : 0;
  const items = list.slice(offset, offset + limit).map(publicTask);
  return Response.json({ items, nextCursor: list.length > offset + limit ? String(offset / limit) : null });
}
