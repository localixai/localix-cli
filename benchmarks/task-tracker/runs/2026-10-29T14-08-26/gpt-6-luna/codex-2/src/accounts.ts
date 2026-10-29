import type { Ctx } from "./context";
import { auth } from "./context";
import { bad, fail, json } from "./http";
import type { User } from "./model";
export function accounts(c:Ctx,method:string,path:string,b:any):Response|undefined {
  const {s}=c;
  if(path==="/users"&&method==="POST"){
    const email=typeof b.email==="string"?b.email.trim().toLowerCase():"";
    if(!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email)) bad("email");
    if(typeof b.password!=="string"||b.password.length<8) bad("password");
    const name=typeof b.name==="string"?b.name.trim():"";
    if(!name||name.length>100) bad("name");
    if(s.users.some(u=>u.email===email)) fail(409,"conflict","Email already taken","email");
    const u:User={id:s.nextUser++,email,name,password:b.password}; s.users.push(u);
    return json({id:u.id,email,name},201);
  }
  if(path==="/sessions"&&method==="POST"){
    const email=typeof b.email==="string"?b.email.toLowerCase():"";
    const u=s.users.find(x=>x.email===email&&x.password===b.password);
    if(!u) return fail(401,"unauthorized","Invalid email or password");
    const token=`token-${u.id}-${s.sessions.size+1}-${c.now()}`; s.sessions.set(token,u.id);
    return json({token},201);
  }
  if(path==="/sessions"&&method==="DELETE"){
    const u=auth(c); const token=(c.request.headers.get("Authorization")||"").slice(7);
    void u; s.sessions.delete(token); return new Response(null,{status:204});
  }
  if(path==="/me"&&method==="GET"){
    const u=auth(c); return json({id:u.id,email:u.email,name:u.name});
  }
}
