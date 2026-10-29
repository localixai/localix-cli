import type { State, Task } from "./model";

export const answer = (body: unknown, status = 200, headers: Record<string, string> = {}) =>
  new Response(status === 204 ? null : JSON.stringify(body), {
    status, headers: { "Content-Type": "application/json", ...headers },
  });
export const fail = (status: number, code: string, message = code, field?: string) =>
  answer({ error: { code, message, ...(field ? { field } : {}) } }, status);
export function member(s: State, p: number, u: number): string | undefined {
  if (s.projects.find(x => x.id === p)?.ownerId === u) return "owner";
  return s.members.get(p)?.get(u);
}
export function access(s: State, p: number, u: number, write = false): boolean {
  const m = member(s, p, u);
  return !!m && (!write || m !== "viewer");
}
export function findTask(s: State, key: string): Task | undefined {
  return s.tasks.find(t => t.key === key);
}
export function taskReply(t: Task, s: State, status = 200) {
  const { deps, history, ...publicTask } = t;
  return answer(publicTask, status, { ETag: `"${t.version}"` });
}
export async function body(req: Request): Promise<Record<string, any> | null> {
  try {
    const value = await req.json();
    return value && typeof value === "object" && !Array.isArray(value) ? value : null;
  } catch { return null; }
}
export const stamp = (s: State) => new Date(s.now()).toISOString();
