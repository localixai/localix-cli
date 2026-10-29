export class HttpError extends Error {
  constructor(public status:number, public code:string, message:string, public field?:string) {
    super(message);
  }
}
export const fail = (status:number, code:string, message:string, field?:string):never => {
  throw new HttpError(status, code, message, field);
};
export const bad = (field:string):never => fail(400,"validation",`Invalid ${field}`,field);
export function json(body:unknown, status=200, headers?:Record<string,string>):Response {
  return Response.json(body,{status,headers});
}
export function errorResponse(error:unknown):Response {
  if (error instanceof HttpError) {
    const value:any={code:error.code,message:error.message};
    if (error.field) value.field=error.field;
    return json({error:value},error.status);
  }
  return json({error:{code:"not_found",message:"Not found"}},404);
}
export function object(value:unknown):Record<string,any> {
  if (!value || typeof value!=="object" || Array.isArray(value)) bad("body");
  return value as Record<string,any>;
}
