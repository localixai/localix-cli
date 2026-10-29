import type { Project, State, Task } from "./model";
import { pair } from "./model";

export class Fault extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
    public field?: string,
  ) {
    super(message);
  }
}
export function fail(status: number, code: string, message: string, field?: string): never {
  throw new Fault(status, code, message, field);
}
export const check: (ok: unknown, field: string, message?: string) => asserts ok = (
  ok,
  field,
  message = "is invalid",
) => {
  if (!ok) fail(400, "validation", `${field} ${message}`, field);
};
export const memberRole = (s: State, p: Project, uid: number) =>
  uid === p.ownerId ? "owner" : s.members.get(pair(p.id, uid));
export const taskFor = (s: State, key: string) => {
  const m = /^([A-Z]{2,6})-(\d+)$/.exec(key);
  if (!m) return undefined;
  const p = s.projects.find((x) => x.key === m[1]);
  return p && s.tasks.find((t) => t.projectId === p.id && t.number === Number(m[2]));
};
export const projectForTask = (s: State, t: Task) => s.projects.find((p) => p.id === t.projectId)!;
export const bodyObject = async (request: Request) => {
  if (!request.headers.get("content-type")?.includes("application/json")) return {};
  try {
    const b = await request.json();
    if (!b || typeof b !== "object" || Array.isArray(b)) return {};
    return b as Record<string, any>;
  } catch {
    return {};
  }
};
export const labels = (value: unknown, field = "labels"): string[] => {
  check(Array.isArray(value), field);
  for (const x of value) check(typeof x === "string" && x.trim().length <= 30 && !/\s/.test(x.trim()), field);
  return [...new Set(value.map((x: string) => x.trim().toLowerCase()))].sort();
};
