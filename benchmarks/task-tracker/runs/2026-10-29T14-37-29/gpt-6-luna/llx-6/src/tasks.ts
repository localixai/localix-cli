import { error, ok, time, type State, type Task } from "./model";
function access(s:State,key:string,uid:number) {
  const t=s.tasks.find(x=>x.key===key), p=t&&s.projects.find(x=>x.id===t.projectId);
  return t&&p?.members.has(uid)?{t,p,role:p.members.get(uid)!}:null;
}
function view(t:Task) {
  const {history,comments,dependsOn,...result}=t;
  void history;void comments;void dependsOn;
  return result;
}
function create(s:State,p:any,uid:number,b:any):Task|string {
  const title=typeof b?.title==="string"?b.title.trim():"";
  if(!title||title.length>200) return "title";
  if(b.description!==undefined&&typeof b.description!=="string") return "description";
  if(b.priority!==undefined&&!["low","normal","high"].includes(b.priority)) return "priority";
  if(b.labels!==undefined&&(!Array.isArray(b.labels)||b.labels.some((x:any)=>
    typeof x!=="string"||!x.trim()||x.trim().length>30||/\s/.test(x.trim())))) return "labels";
  if(b.assigneeId!==undefined&&b.assigneeId!==null&&!p.members.has(b.assigneeId)) return "assigneeId";
  const n=(s.sequences.get(p.id)||0)+1;
  s.sequences.set(p.id,n);
  const labels=[...new Set<string>((b.labels||[]).map((x:string)=>x.trim().toLowerCase()))].sort();
  const t:Task={
    id:++s.ids.task,key:`${p.key}-${n}`,number:n,projectId:p.id,title,
    description:b.description||"",status:"open",priority:b.priority||"normal",
    labels,assigneeId:b.assigneeId??null,createdBy:uid,createdAt:time(s),
    updatedAt:time(s),version:1,dependsOn:[],history:[],comments:[],
  };
  s.tasks.push(t); return t;
}
export function tasks(s:State,method:string,path:string,b:any,uid:number,headers:Headers) {
  const query = new URLSearchParams(path.split("?")[1] || "");
  path = path.split("?")[0]!;
  const pc=path.match(/^\/projects\/(\d+)\/tasks(?:\/(bulk))?$/);
  if(pc) {
    const p=s.projects.find(x=>x.id===Number(pc[1]));
    if(!p||!p.members.has(uid)) return error("not_found","Not found",404);
    if(method==="GET") {
      let list=s.tasks.filter(t=>t.projectId===p.id);
      const q=query;
      if(q.has("status")&&!["open","in_progress","done"].includes(q.get("status")!))
        return error("validation","Invalid status",400,"status");
      if(q.has("status")) list=list.filter(t=>t.status===q.get("status"));
      if(q.has("assignee")) {
        const assignee=q.get("assignee")!;
        if(assignee==="me")list=list.filter(t=>t.assigneeId===uid);
        else if(assignee==="none")list=list.filter(t=>t.assigneeId===null);
        else if(!/^\d+$/.test(assignee))return error("validation","Invalid assignee",400,"assignee");
        else list=list.filter(t=>t.assigneeId===Number(assignee));
      }
      if(q.has("label")) list=list.filter(t=>t.labels.includes(q.get("label")!.toLowerCase()));
      if(q.has("q")) list=list.filter(t=>(t.title+t.description).toLowerCase().includes(q.get("q")!.toLowerCase()));
      const sort=q.get("sort")||"";
      if(!["","-created","priority"].includes(sort))return error("validation","Invalid sort",400,"sort");
      if(sort==="-created")list.reverse();
      if(sort==="priority")list.sort((a,b)=>
        ["high","normal","low"].indexOf(a.priority)-["high","normal","low"].indexOf(b.priority)||
        a.number-b.number);
      const raw=q.get("limit")||"20",limit=Number(raw);
      if(!/^\d+$/.test(raw)||limit<1||limit>100)return error("validation","Invalid limit",400,"limit");
      const cursor=q.get("cursor"),offset=cursor?Number(cursor):0;
      if(cursor!==null&&(!/^\d+$/.test(cursor)||offset>=list.length))
        return error("validation","Invalid cursor",400,"cursor");
      const items=list.slice(offset,offset+limit),next=offset+limit<list.length?String(offset+limit):null;
      return ok({items,nextCursor:next});
    }
    if(method==="POST") {
      if(p.members.get(uid)==="viewer") return error("forbidden","Forbidden",403);
      if(pc[2]==="bulk") {
        if(!Array.isArray(b?.tasks)||!b.tasks.length||b.tasks.length>50)
          return error("validation","Invalid tasks",400,"tasks");
        const made=[]; const before=s.tasks.length, sequence=s.sequences.get(p.id)||0, taskId=s.ids.task;
        for(let i=0;i<b.tasks.length;i++) {
          const t=create(s,p,uid,b.tasks[i]);
          if(typeof t==="string") {
            s.tasks.length=before;s.sequences.set(p.id,sequence);s.ids.task=taskId;
            return error("validation","Invalid task",400,`tasks[${i}].${t}`);
          }
          made.push(view(t));
        }
        return ok({items:made},201);
      }
      const ik=headers.get("Idempotency-Key"), id=ik?`${uid}:${ik}`:"";
      const old=id?s.idem.get(id):undefined;
      if(old&&s.now()-old.time<86400000) {
        if(old.body!==JSON.stringify(b))return error("idempotency_mismatch","Mismatch",422);
        return ok(view(old.task),201,{"Idempotent-Replayed":"true","ETag":`"${old.task.version}"`});
      }
      const t=create(s,p,uid,b);
      if(typeof t==="string")return error("validation","Invalid task",400,t);
      if(ik)s.idem.set(id,{body:JSON.stringify(b),task:t,time:s.now()});
      return ok(view(t),201,{"ETag":`"${t.version}"`});
    }
  }
  const match=path.match(/^\/tasks\/([A-Z]+-\d+)(?:\/(.*))?$/); if(!match)return null;
  const a=access(s,match[1]!,uid); if(!a)return error("not_found","Not found",404);
  const {t,p,role}=a, sub=match[2];
  if(!sub&&method==="GET")return ok(view(t),200,{"ETag":`"${t.version}"`});
  if(sub==="comments") {
    if(method==="GET")return ok(t.comments);
    if(method==="POST") {
      const text=typeof b?.body==="string"?b.body.trim():"";
      if(!text||text.length>2000)return error("validation","Invalid comment",400,"body");
      const c={id:++s.ids.comment,authorId:uid,body:text,createdAt:time(s)};
      t.comments.push(c);return ok(c,201);
    }
  }
  if(!sub&&method==="DELETE") {
    if(role!=="owner"&&t.createdBy!==uid)return error("forbidden","Forbidden",403);
    s.tasks=s.tasks.filter(x=>x!==t);
    s.tasks.forEach(x=>x.dependsOn=x.dependsOn.filter(k=>k!==t.key));
    return ok(null,204);
  }
  if(!sub&&method==="PATCH") {
    if(role==="viewer")return error("forbidden","Forbidden",403);
    const allowed=["title","description","priority","labels","assigneeId"];
    const unknown=Object.keys(b||{}).find(k=>!allowed.includes(k));
    if(unknown)return error("validation","Unknown field",400,unknown);
    if(b.title!==undefined&&(typeof b.title!=="string"||!b.title.trim()||b.title.trim().length>200))
      return error("validation","Invalid title",400,"title");
    if(b.description!==undefined&&typeof b.description!=="string")
      return error("validation","Invalid description",400,"description");
    if(b.priority!==undefined&&!["low","normal","high"].includes(b.priority))
      return error("validation","Invalid priority",400,"priority");
    if(b.labels!==undefined&&(!Array.isArray(b.labels)||b.labels.some((x:any)=>
      typeof x!=="string"||!x.trim()||x.trim().length>30||/\s/.test(x.trim()))))
      return error("validation","Invalid labels",400,"labels");
    if(b.assigneeId!==undefined&&b.assigneeId!==null&&!p.members.has(b.assigneeId))
      return error("validation","Invalid assignee",400,"assigneeId");
    const old={...t};
    for(const k of ["title","description","priority","labels","assigneeId"])
      if(b?.[k]!==undefined)t[k as keyof Task]=b[k] as never;
    if(b.title!==undefined)t.title=b.title.trim();
    if(b.labels!==undefined)t.labels=[...new Set<string>(b.labels.map((x:string)=>x.trim().toLowerCase()))].sort();
    if(headers.has("If-Match")&&headers.get("If-Match")!==`"${old.version}"`) {
      Object.assign(t,old);
      return error("precondition_failed","Changed",412);
    }
    const fields=["assigneeId","labels","priority","title","description"];
    const changed=fields.filter(k=>JSON.stringify(old[k as keyof Task])!==JSON.stringify(t[k as keyof Task]));
    if(!changed.length)return ok(view(t));
    t.version++;t.updatedAt=time(s);
    for(const k of changed)
      t.history.push({at:t.updatedAt,by:uid,field:k,from:old[k as keyof Task],to:t[k as keyof Task]});
    return ok(view(t),200,{"ETag":`"${t.version}"`});
  }
  if(sub==="transitions"&&method==="POST"){
    if(role==="viewer")return error("forbidden","Forbidden",403);
    if(headers.has("If-Match")&&headers.get("If-Match")!==`"${t.version}"`)
      return error("precondition_failed","Changed",412);
    if(!["open","in_progress","done"].includes(b?.to))return error("validation","Invalid status",400,"to");
    const valid=(t.status==="open"&&b.to==="in_progress")||
      (t.status==="in_progress"&&["open","done"].includes(b.to))||
      (t.status==="done"&&b.to==="open");
    if(!valid)return error("invalid_transition","Invalid transition",409);
    if(b.to==="done"&&t.dependsOn.some(k=>s.tasks.find(x=>x.key===k)?.status!=="done"))
      return error("blocked","Blocked",409);
    const from=t.status;
    t.status=b.to;t.version++;t.updatedAt=time(s);
    t.history.push({at:t.updatedAt,by:uid,field:"status",from,to:b.to});
    return ok(view(t),200,{"ETag":`"${t.version}"`});
  }
  if(sub==="history"&&method==="GET")return ok(t.history);
  if(sub==="dependencies"&&method==="GET") {
    const byNumber=(a:string,b:string)=>Number(a.split("-")[1])-Number(b.split("-")[1]);
    const dependsOn=t.dependsOn.slice().sort(byNumber);
    const blocks=s.tasks.filter(x=>x.dependsOn.includes(t.key)).map(x=>x.key).sort(byNumber);
    return ok({key:t.key,dependsOn,blocks});
  }
  if(sub==="dependencies"&&method==="POST") {
    if(role==="viewer")return error("forbidden","Forbidden",403);
    const key=b?.dependsOn, other=s.tasks.find(x=>x.key===key&&x.projectId===p.id);
    if(typeof key!=="string"||!other||other===t)return error("validation","Invalid dependency",400,"dependsOn");
    if(t.dependsOn.includes(key))return error("conflict","Already depends",409);
    const reaches=(from:Task,target:string,seen=new Set<string>()):boolean=>{
      if(from.key===target)return true;
      if(seen.has(from.key))return false;
      seen.add(from.key);
      return from.dependsOn.some(k=>{const next=s.tasks.find(x=>x.key===k);return !!next&&reaches(next,target,seen);});
    };
    if(reaches(other,t.key))return error("dependency_cycle","Dependency cycle",409);
    t.dependsOn.push(key);
    const dependsOn=t.dependsOn.slice().sort((a,b)=>Number(a.split("-")[1])-Number(b.split("-")[1]));
    return ok({key:t.key,dependsOn,blocks:[]},201);
  }
  const remove=String(sub||"").match(/^dependencies\/([A-Z]+-\d+)$/);
  if(remove&&method==="DELETE") {
    if(role==="viewer")return error("forbidden","Forbidden",403);
    if(!t.dependsOn.includes(remove[1]!))return error("not_found","Not found",404);
    t.dependsOn=t.dependsOn.filter(k=>k!==remove[1]);return ok(null,204);
  }
  return null;
}
