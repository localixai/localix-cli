import { handle } from "./router";
import { state } from "./state";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };

export function createApp(options: Options = {}): App {
  const db = state(options.now ?? Date.now);
  return { fetch: (request) => handle(request, db) };
}
