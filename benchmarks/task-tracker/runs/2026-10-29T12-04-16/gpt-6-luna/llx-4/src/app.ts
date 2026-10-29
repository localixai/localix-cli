import { state } from "./model";
import { auth, userRoutes } from "./auth";
import { projectRoutes } from "./projects";
import { taskRoutes } from "./task-routes";
import { Fault, fail } from "./util";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };

const routeMethods: [RegExp, string][] = [
  [/^\/users$/, "POST"], [/^\/sessions$/, "POST, DELETE"], [/^\/me$/, "GET"],
  [/^\/projects$/, "GET, POST"], [/^\/projects\/\d+$/, "GET"],
  [/^\/projects\/\d+\/(members|stats)$/, "GET, POST"],
  [/^\/projects\/\d+\/members\/\d+$/, "DELETE"],
  [/^\/projects\/\d+\/tasks$/, "GET, POST"], [/^\/projects\/\d+\/tasks\/bulk$/, "POST"],
  [/^\/tasks\/[A-Z]+-\d+$/, "GET, PATCH, DELETE"],
  [/^\/tasks\/[A-Z]+-\d+\/(comments|history|dependencies)$/, "GET, POST"],
  [/^\/tasks\/[A-Z]+-\d+\/(transitions)$/, "POST"],
  [/^\/tasks\/[A-Z]+-\d+\/dependencies\/.+$/, "DELETE"],
];

export function createApp(options: Options = {}): App {
  const s = state(), clock = options.now ?? Date.now;
  return { async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url), path = url.pathname, method = request.method;
    const matched = routeMethods.find(([re]) => re.test(path));
    if (!matched) return respond(404,{error:{code:"not_found",message:"Not found"}});
    if (!matched[1].split(", ").includes(method)) {
      const r=respond(405,{error:{code:"method_not_allowed",message:"Method not allowed"}});
      r.headers.set("Allow",matched[1]);return r;
    }
    const authorization=request.headers.get("Authorization")??"";
    const token=authorization.startsWith("Bearer ")?authorization.slice(7):undefined;
    let body:unknown;
    if (["POST","PATCH","PUT"].includes(method)) {
      try { body=await request.json(); } catch { return error(new Fault(400,"validation","Invalid JSON")); }
      if (!body || typeof body !== "object" || Array.isArray(body))
        return error(new Fault(400,"validation","Expected an object"));
    }
    const at=new Date(clock()).toISOString();
    try {
      // Authenticate protected routes before project/task lookup.
      if (!["/users","/sessions"].includes(path) && !(path==="/sessions"&&method==="POST")) auth(s,token);
      let limitInfo: { minute: number; used: number } | undefined;
      if (token && s.sessions.has(token) && path!=="/sessions" && path!=="/users") {
        const minute=Math.floor(clock()/60000),old=s.limits.get(token);
        const used=old?.minute===minute?old.used:0;
        if (used>=100) {
          const wait=Math.ceil(((minute+1)*60000-clock())/1000);
          const limited=respond(429,{error:{code:"rate_limited",message:"Rate limit exceeded"}});
          limited.headers.set("Retry-After",String(wait));
          limited.headers.set("X-RateLimit-Limit","100");
          limited.headers.set("X-RateLimit-Remaining","0");
          return limited;
        }
        limitInfo={minute,used};
      }
      const result=userRoutes(s,method,path,body,token) ??
        projectRoutes(s,method,path,body,token,at) ??
        taskRoutes(s,method,path,body,token,at,url,request.headers);
      if (!result) return respond(404,{error:{code:"not_found",message:"Not found"}});
      const [status,data] = result as [number,unknown,Record<string,string>?];
      let headers = (result as [number,unknown,Record<string,string>?])[2];
      if (!headers && data && typeof data === "object" && "version" in data)
        headers = {ETag: `"${(data as {version:number}).version}"`};
      let response=respond(status,data,headers);
      if(limitInfo && token) {
        const {minute,used}=limitInfo;
        response.headers.set("X-RateLimit-Limit","100");
        response.headers.set("X-RateLimit-Remaining",String(99-used));
        s.limits.set(token,{minute,used:used+1});
      }
      return response;
    } catch(e) { return error(e instanceof Fault?e:new Fault(500,"internal","Internal server error")); }
  }};
}
function respond(status:number,data:unknown,headers:Record<string,string>={}) {
  const response=status===204?new Response(null,{status}):Response.json(data,{status});
  for(const [key,value] of Object.entries(headers))response.headers.set(key,value);
  return response;
}
function error(e:Fault) {
  const detail:Record<string,string>={code:e.code,message:e.message};
  if(e.field)detail.field=e.field;
  return respond(e.status,{error:detail});
}
