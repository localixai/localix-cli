import type { Store } from "./model";
import { route } from "./router";
import { errorReply } from "./http";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
export function createApp(options: Options = {}): App {
  const store: Store = {
    users: [], projects: [], tasks: [], sessions: new Map(), limits: new Map(),
    idem: new Map(), now: options.now ?? Date.now, tokenSeq: 0,
  };
  return { async fetch(request) {
    try { return await route(store, request); }
    catch (error) { return errorReply(error); }
  } };
}
