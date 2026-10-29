export type User = { id:number; email:string; name:string; password:string };
export type Project = { id:number; name:string; key:string; ownerId:number; createdAt:string };
export type Task = {
  id:number; number:number; projectId:number; title:string; description:string;
  key:string;
  status:string; priority:string; labels:string[]; assigneeId:number|null;
  createdBy:number; createdAt:string; updatedAt:string; version:number;
};
export type History = { at:string; by:number; field:string; from:unknown; to:unknown };
export type Store = {
  users:User[]; sessions:Map<string,number>; projects:Project[];
  members:Map<number,Map<number,string>>; tasks:Task[];
  histories:Map<number,History[]>; comments:Map<number, any[]>;
  deps:Map<number,Set<number>>; nextUser:number; nextProject:number;
  nextTask:number; nextComment:number; idempotency:Map<string,any>;
  taskNumbers:Map<number,number>;
  rates:Map<string,{minute:number; count:number}>;
};
export function store():Store {
  return { users:[], sessions:new Map(), projects:[], members:new Map(), tasks:[],
    histories:new Map(), comments:new Map(), deps:new Map(), nextUser:1, nextProject:1,
    nextTask:1, nextComment:1, idempotency:new Map(), taskNumbers:new Map(), rates:new Map() };
}
