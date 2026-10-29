import { State } from "./state";
import { handle } from "./routes";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };

export function createApp(options: Options = {}): App {
  const state = new State(options.now ?? Date.now);
  return { fetch: (request) => handle(request, state) };
}
