export type Options = { now?: () => number };
export type App = { fetch(request: Request): Response | Promise<Response> };
