export type Options = {
  /** The current time in milliseconds since the epoch; `Date.now` when not given. */
  now?: () => number;
};

export type App = {
  fetch(request: Request): Response | Promise<Response>;
};

export function createApp(options: Options = {}): App {
  void options;
  return {
    fetch() {
      return Response.json({ error: { code: "not_implemented", message: "not implemented" } }, { status: 501 });
    },
  };
}
