import type { State, Task } from "./model";
import { auth } from "./auth";
import { addTask, fail, membership, need, obj, taskByKey } from "./util";

function access(s: State, key: string, token?: string) {
  const uid = auth(s, token), t = taskByKey(s, key);
  if (!t || !membership(s, t.projectId, uid)) fail(404, "not_found", "Task not found");
  return { uid, t };
}
function canWrite(s: State, t: Task, uid: number) {
  const role = membership(s,t.projectId,uid);
  if (role !== "owner" && role !== "member") fail(403,"forbidden","Member access required");
}
function etag(t: Task, headers: Headers) {
  const match = headers.get("If-Match");
  if (match && match !== `"${t.version}"`) fail(412,"precondition_failed","Task has changed");
}
function record(s: State,t:Task,uid:number,at:string,field:string,from:any,to:any) {
  s.history.get(t.id)!.push({at,by:uid,field,from,to});
}
function update(s:State,t:Task,uid:number,at:string,b:Record<string,any>) {
  const allowed=["assigneeId","description","labels","priority","title"];
  for(const key of Object.keys(b)) need(allowed.includes(key),""+key);
  const next:Record<string,any>={};
  if ("title" in b) {
    need(typeof b.title==="string"&&b.title.trim().length>0&&b.title.trim().length<=200,"title");
    next.title=b.title.trim();
  }
  if ("description" in b) { need(typeof b.description==="string","description"); next.description=b.description; }
  if ("priority" in b) { need(["low","normal","high"].includes(b.priority),"priority"); next.priority=b.priority; }
  if ("labels" in b) {
    need(Array.isArray(b.labels),"labels");
    for(const x of b.labels) need(typeof x==="string"&&/^[a-zA-Z0-9_-]{1,30}$/.test(x.trim()),"labels");
    next.labels=[...new Set(b.labels.map((x:string)=>x.trim().toLowerCase()))].sort();
  }
  if ("assigneeId" in b) {
    need(b.assigneeId===null || (Number.isInteger(b.assigneeId)&&membership(s,t.projectId,b.assigneeId)),"assigneeId");
    next.assigneeId=b.assigneeId;
  }
  let changed=false;
  for(const key of Object.keys(next).sort()) {
    if (JSON.stringify((t as any)[key])!==JSON.stringify(next[key])) {
      record(s,t,uid,at,key,(t as any)[key],next[key]); (t as any)[key]=next[key]; changed=true;
    }
  }
  if(changed){t.version++;t.updatedAt=at;}
  return changed;
}
function listing(s:State,projectId:number,uid:number,url:URL) {
  let items=s.tasks.filter(t=>t.projectId===projectId);
  const q=url.searchParams;
  const status=q.get("status"),assignee=q.get("assignee"),label=q.get("label"),search=q.get("q");
  if(status) { need(["open","in_progress","done"].includes(status),"status");items=items.filter(t=>t.status===status); }
  if(assignee) {
    need(assignee==="me"||assignee==="none"||/^\d+$/.test(assignee),"assignee");
    items=items.filter(t=>assignee==="none"?t.assigneeId===null:t.assigneeId===(assignee==="me"?uid:Number(assignee)));
  }
  if(label) items=items.filter(t=>t.labels.includes(label.toLowerCase()));
  if(search) items=items.filter(t=>(t.title+" "+t.description).toLowerCase().includes(search.toLowerCase()));
  const sort=q.get("sort")??"created";
  need(["created","-created","priority"].includes(sort),"sort");
  items.sort(sort==="priority"
    ? (a,b)=>["high","normal","low"].indexOf(a.priority)-["high","normal","low"].indexOf(b.priority)||a.number-b.number
    : (a,b)=>sort==="-created"?b.number-a.number:a.number-b.number);
  let limit=q.has("limit")?Number(q.get("limit")):20;
  need(Number.isInteger(limit)&&limit>=1&&limit<=100,"limit");
  if(q.has("cursor")) {
    let c:any;try{c=JSON.parse(atob(q.get("cursor")!));}catch{fail(400,"validation","Invalid cursor","cursor");}
    need(c&&c.last,"cursor");
    const idx=items.findIndex(t=>t.key===c.last);
    need(idx>=0,"cursor");items=items.slice(idx+1);
  }
  const more=items.length>limit, page=items.slice(0,limit);
  return {items:page,nextCursor:more?btoa(JSON.stringify({last:page[page.length-1]!.key})):null};
}
export function taskRoutes(
  s:State,method:string,path:string,body:any,token:string|undefined,now:string,url:URL,headers:Headers,
) {
  let m=path.match(/^\/projects\/(\d+)\/tasks(?:\/(bulk))?$/);
  if(m) {
    const projectId=Number(m[1]),uid=auth(s,token),p=s.projects.find(x=>x.id===projectId);
    const role=p&&membership(s,projectId,uid);
    if(!p||!role) fail(404,"not_found","Project not found");
    if(method==="GET"&&!m[2]) return [200,listing(s,projectId,uid,url)];
    if(method==="POST") {
      canWrite(s,{projectId} as Task,uid);
      const b=obj(body), bulk=!!m[2], inputs=bulk?b.tasks:[b];
      if(bulk) need(Array.isArray(inputs)&&inputs.length>=1&&inputs.length<=50,"tasks");
      if(bulk) {
        const validated:any[]=[];
        for(let i=0;i<inputs.length;i++) try {
          validateTask(s,projectId,inputs[i]);
          validated.push(inputs[i]);
        } catch(e) {
          if(e instanceof Error && "field" in e)
            (e as any).field=`tasks[${i}].${(e as any).field}`;
          throw e;
        }
        return [201,{items:validated.map(x=>addTask(s,projectId,uid,x,now))}];
      }
      validateTask(s,projectId,b);
      const ik=headers.get("Idempotency-Key");
      if(ik) {
        const k=`${uid}:${ik}`, saved=s.idem.get(k), signature=JSON.stringify(b);
        if(saved&&nowMs(now)-saved.at<86400000) {
          if(saved.body!==signature) fail(422,"idempotency_mismatch","Key used with another body");
          return [201,saved.task,{ "Idempotent-Replayed":"true" }];
        }
        const task=addTask(s,projectId,uid,b,now);s.idem.set(k,{at:nowMs(now),body:signature,task});
        return [201,task];
      }
      return [201,addTask(s,projectId,uid,b,now)];
    }
  }
  m=path.match(/^\/tasks\/([A-Z]+-\d+)(?:\/(comments|history|transitions|dependencies)(?:\/(.+))?)?$/);
  if(!m)return null;
  const {uid,t}=access(s,m[1]!,token),sub=m[2],extra=m[3];
  if(!sub&&method==="GET")return [200,t,{"ETag":`"${t.version}"`}];
  if(!sub&&method==="DELETE") {
    if(membership(s,t.projectId,uid)!=="owner"&&t.createdBy!==uid)
      fail(403,"forbidden","Only the creator or owner may delete");
    s.tasks=s.tasks.filter(x=>x!==t);
    for(const deps of s.deps.values())deps.delete(t.id);
    s.deps.delete(t.id);return [204,null];
  }
  if(!sub&&method==="PATCH") {
    canWrite(s,t,uid);etag(t,headers);update(s,t,uid,now,obj(body));return [200,t,{"ETag":`"${t.version}"`}];
  }
  if(sub==="history"&&method==="GET")return [200,s.history.get(t.id)];
  if(sub==="comments") {
    if(method==="GET")return [200,s.comments.get(t.id)];
    if(method==="POST") {
      const b=obj(body),text=typeof b.body==="string"?b.body.trim():"";
      need(text.length>0&&text.length<=2000,"body");
      const c={id:++s.ids.comment,authorId:uid,body:text,createdAt:now};s.comments.get(t.id)!.push(c);return [201,c];}
  }
  if(sub==="transitions"&&method==="POST") {
    canWrite(s,t,uid);etag(t,headers);const b=obj(body);need(["open","in_progress","done"].includes(b.to),"to");
    const allowed:Record<string,string[]>={open:["in_progress"],in_progress:["open","done"],done:["open"]};
    if(!allowed[t.status]!.includes(b.to))fail(409,"invalid_transition","Invalid transition");
    if(b.to==="done"&&[...(s.deps.get(t.id)??[])].some(
      id=>s.tasks.some(x=>x.id===id&&x.status!=="done")))
      fail(409,"blocked","Dependencies are not done");
    record(s,t,uid,now,"status",t.status,b.to);t.status=b.to;t.version++;t.updatedAt=now;
    return [200,t,{"ETag":`"${t.version}"`}];
  }
  return dependencyRoutes(s,method,t,sub,extra,body,uid);
}
function validateTask(s:State,p:number,b:any) { obj(b);addTaskValidation(s,p,b); }
function addTaskValidation(s:State,p:number,b:any) {
  const title=typeof b.title==="string"?b.title.trim():"";need(title.length>0&&title.length<=200,"title");
  if(b.description!==undefined)need(typeof b.description==="string","description");
  if(b.priority!==undefined)need(["low","normal","high"].includes(b.priority),"priority");
  if(b.labels!==undefined){
    need(Array.isArray(b.labels),"labels");
    for(const l of b.labels)
      need(typeof l==="string"&&/^[a-zA-Z0-9_-]{1,30}$/.test(l.trim()),"labels");
  }
  if(b.assigneeId!==undefined&&b.assigneeId!==null)
    need(Number.isInteger(b.assigneeId)&&membership(s,p,b.assigneeId),"assigneeId");
}
function dependencyRoutes(s:State,method:string,t:Task,sub:string|undefined,extra:string|undefined,b:any,uid:number) {
  if(sub!=="dependencies")return null;
  if(method==="GET") {
    const deps=s.deps.get(t.id)??new Set();
    return [200,{key:t.key,
      dependsOn:[...deps].map(id=>s.tasks.find(x=>x.id===id)!.key)
        .sort((a,b)=>Number(a.split("-")[1])-Number(b.split("-")[1])),
      blocks:s.tasks.filter(x=>s.deps.get(x.id)?.has(t.id)).sort((a,b)=>a.number-b.number).map(x=>x.key)}];
  }
  if(method==="DELETE"&&extra) {
    const other=taskByKey(s,extra),deps=s.deps.get(t.id)!;
    if(!other||!deps.delete(other.id))fail(404,"not_found","Dependency not found");
    return [204,null];
  }
  if(method==="POST") {
    canWrite(s,t,uid);const bdy=obj(b);need(typeof bdy.dependsOn==="string","dependsOn");
    const other=taskByKey(s,bdy.dependsOn);
    if(!other||other.id===t.id||other.projectId!==t.projectId)fail(400,"validation","Invalid dependency","dependsOn");
    const deps=s.deps.get(t.id)!;
    if(deps.has(other.id))fail(409,"conflict","Dependency exists");
    const reaches=(from:number,target:number,seen=new Set<number>()):boolean=>{
      if(from===target)return true;if(seen.has(from))return false;seen.add(from);
      return [...(s.deps.get(from)??[])].some(id=>reaches(id,target,seen));
    };
    if(reaches(other.id,t.id))fail(409,"dependency_cycle","Dependency cycle");
    deps.add(other.id);
    return [201,{key:t.key,dependsOn:[...deps].map(id=>s.tasks.find(x=>x.id===id)!.key).sort(),blocks:[]}];
  }
  return null;
}
const nowMs=(s:string)=>Date.parse(s);
