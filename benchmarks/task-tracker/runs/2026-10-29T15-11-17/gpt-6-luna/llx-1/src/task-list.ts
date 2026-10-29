import type { RequestCtx } from "./http";
import { bad, ok } from "./http";
import { memberRole } from "./model";

export function listTasks(path: string, c: RequestCtx) {
  const m = path.match(/^\/projects\/(\d+)\/tasks$/); if (!m) return undefined;
  const pid = Number(m[1]);
  if (!memberRole(c.state,pid,c.userId)) return { status:404, body:{error:{code:"not_found",message:"Not found"}} };
  const q = c.url.searchParams, status = q.get("status"), sort = q.get("sort") ?? "created";
  const rawLimit = q.get("limit") ?? "20", limit = Number(rawLimit);
  if (status && !["open","in_progress","done"].includes(status)) return bad("status");
  if (!["created","-created","priority"].includes(sort)) return bad("sort");
  if (!/^\d+$/.test(rawLimit) || limit < 1 || limit > 100) return bad("limit");
  const assignee = q.get("assignee");
  if (assignee && !["me","none"].includes(assignee) && !/^\d+$/.test(assignee)) return bad("assignee");
  let a = c.state.tasks.filter(t => t.projectId===pid && (!status || t.status===status));
  if (assignee === "me") a=a.filter(t=>t.assigneeId===c.userId);
  else if (assignee === "none") a=a.filter(t=>t.assigneeId===null);
  else if (assignee) a=a.filter(t=>t.assigneeId===Number(assignee));
  const label = q.get("label")?.toLowerCase(), search = q.get("q")?.toLowerCase();
  if (label) a=a.filter(t=>t.labels.includes(label));
  if (search) a=a.filter(t=>(t.title+" "+t.description).toLowerCase().includes(search));
  const rank = { high:0, normal:1, low:2 } as Record<string,number>;
  a.sort((x,y)=>sort==="priority" ? rank[x.priority]!-rank[y.priority]! || x.number-y.number :
    sort==="-created" ? y.number-x.number : x.number-y.number);
  const cursor = q.get("cursor");
  if (cursor) {
    try {
      const n=Number(atob(cursor));
      if (!Number.isInteger(n)) throw 0;
      a=a.slice(n);
    } catch { return bad("cursor"); }
  }
  const items=a.slice(0,limit), offset=(cursor ? Number(atob(cursor)) : 0)+items.length;
  return ok({ items, nextCursor: offset<a.length ? btoa(String(offset)) : null });
}
