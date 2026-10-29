import type { Ctx } from "./context";
import { auth, iso, project, visibleProject } from "./context";
import { bad, fail, json } from "./http";
export function projects(c:Ctx,method:string,path:string,b:any):Response|undefined {
  const {s}=c;
  const stats=/^\/projects\/(\d+)\/stats$/.exec(path);
  if(stats&&method==="GET"){
    const p=visibleProject(c,Number(stats[1]));
    const tasks=s.tasks.filter(t=>t.projectId===p.id);
    const byStatus={open:0,in_progress:0,done:0};
    const byPriority={high:0,normal:0,low:0};
    const counts=new Map<number,number>();
    for(const t of tasks){
      (byStatus as any)[t.status]++; (byPriority as any)[t.priority]++;
      if(t.assigneeId!==null)counts.set(t.assigneeId,(counts.get(t.assigneeId)||0)+1);
    }
    const byAssignee=[...counts].sort((a,b)=>b[1]-a[1]||a[0]-b[0])
      .map(([userId,count])=>({userId,count}));
    return json({total:tasks.length,byStatus,byPriority,byAssignee});
  }
  if(path==="/projects"&&method==="POST"){
    const u=auth(c), name=typeof b.name==="string"?b.name.trim():"";
    if(!name||name.length>100) bad("name");
    if(typeof b.key!=="string"||!/^[A-Z]{2,6}$/.test(b.key)) bad("key");
    if(s.projects.some(p=>p.key===b.key)) fail(409,"conflict","Project key taken","key");
    const p={id:s.nextProject++,name,key:b.key,ownerId:u.id,createdAt:iso(c)};
    s.projects.push(p); s.members.set(p.id,new Map([[u.id,"owner"]])); return json(p,201);
  }
  if(path==="/projects"&&method==="GET"){
    const u=auth(c); return json(s.projects.filter(p=>s.members.get(p.id)?.has(u.id)));
  }
  let m=/^\/projects\/([^/]+)(?:\/(members)(?:\/([^/]+))?)?$/.exec(path);
  if(m){
    const id=/^\d+$/.test(m[1]||"")?Number(m[1]):NaN;
    if(!Number.isFinite(id)) fail(404,"not_found","Project not found");
    const p=visibleProject(c,id), members=s.members.get(id)!;
    if(!m[2]&&method==="GET") return json(p);
    if(m[2]&&method==="GET") return json([...members].sort((a,b)=>a[0]-b[0]).map(([userId,role])=>({userId,role})));
    if(m[2]&&method==="POST"){
      if(auth(c).id!==p.ownerId) fail(403,"forbidden","Forbidden");
      if(!Number.isInteger(b.userId)||!s.users.some(u=>u.id===b.userId)) bad("userId");
      if(!["member","viewer"].includes(b.role)) bad("role");
      if(members.has(b.userId)) fail(409,"conflict","Already a member");
      members.set(b.userId,b.role); return json({userId:b.userId,role:b.role},201);
    }
    if(m[2]&&m[3]&&method==="DELETE"){
      if(auth(c).id!==p.ownerId) fail(403,"forbidden","Forbidden");
      const uid=Number(m[3]); if(!members.has(uid)||uid===p.ownerId) fail(404,"not_found","Member not found");
      members.delete(uid); for(const t of s.tasks) if(t.projectId===id&&t.assigneeId===uid) t.assigneeId=null;
      return new Response(null,{status:204});
    }
  }
}
