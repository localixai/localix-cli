import { route, type Context } from "./router";
import { error } from "./shared";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };

export function createApp(options: Options = {}): App {
  const c: Context = {
    now: options.now ?? Date.now, users: [], projects: [], tasks: [], sessions: new Map(),
    members: new Map(), comments: [], dependencies: new Map(), history: new Map(),
    idem: new Map(), rates: new Map(), ids: {
      user: 0, project: 0, task: 0, comment: 0, number: new Map(),
    },
  };
  return { async fetch(req) {
    try {
      return await route(c, req);
    } catch {
      return error(400, "validation", "Invalid request");
    }
  } };
}
