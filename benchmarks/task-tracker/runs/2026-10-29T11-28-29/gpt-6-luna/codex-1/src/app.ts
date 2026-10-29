import { createRouter } from "./router";
import type { State } from "./model";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
export function createApp(options: Options = {}): App {
  const s: State = {
    users: [],
    projects: [],
    tasks: [],
    sessions: new Map(),
    members: new Map(),
    comments: new Map(),
    history: new Map(),
    deps: new Map(),
    idempotency: new Map(),
    limits: new Map(),
    ids: { user: 0, project: 0, task: 0, comment: 0 },
    now: options.now ?? Date.now,
  };
  return { fetch: createRouter(s) };
}
