import { handle } from "./server";
import { Store } from "./store";

export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };

export function createApp(options: Options = {}): App {
  const store = new Store(options.now ?? Date.now);
  return { fetch: (request) => handle(request, store) };
}
