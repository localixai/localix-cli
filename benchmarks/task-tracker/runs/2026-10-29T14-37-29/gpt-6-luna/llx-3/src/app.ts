import { createState } from "./state";
import { route } from "./routes";
export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };

export function createApp(options: Options = {}): App {
  const state = createState(options.now ?? Date.now);
  return { fetch: (request) => route(request, state) };
}
