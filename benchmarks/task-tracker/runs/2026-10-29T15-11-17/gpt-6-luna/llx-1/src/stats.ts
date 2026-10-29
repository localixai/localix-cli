import type { RequestCtx } from "./http";
import { memberRole } from "./model";

export function stats(path:string,c:RequestCtx) {
  const m=path.match(/^\/projects\/(\d+)\/stats$/);if(!m)return undefined;
  const id=Number(m[1]);if(!memberRole(c.state,id,c.userId))
    return {status:404,body:{error:{code:"not_found",message:"Not found"}}};
  const tasks=c.state.tasks.filter(t=>t.projectId===id);
  const byStatus={open:0,in_progress:0,done:0},byPriority={high:0,normal:0,low:0};
  const assigned=new Map<number,number>();
  for(const t of tasks){byStatus[t.status as keyof typeof byStatus]++;
    byPriority[t.priority as keyof typeof byPriority]++;
    if(t.assigneeId!==null)assigned.set(t.assigneeId,(assigned.get(t.assigneeId)??0)+1);}
  const byAssignee=[...assigned].map(([userId,count])=>({userId,count}))
    .sort((a,b)=>b.count-a.count||a.userId-b.userId);
  return {status:200,body:{total:tasks.length,byStatus,byPriority,byAssignee}};
}
