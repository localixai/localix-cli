import { createApp, type App } from "../src/app";

/** 2026-01-01T00:00:00.000Z: the start of a minute. */
export const START = Date.UTC(2026, 0, 1);

export type Reply = { status: number; body: any; headers: Headers };

export type Call = {
  body?: unknown;
  /** A raw body, sent as it is (for bodies that are not JSON). */
  raw?: string;
  token?: string;
  headers?: Record<string, string>;
};

/** An app on a clock the test moves, and a way to call it. */
export function setup() {
  let now = START;
  const app: App = createApp({ now: () => now });
  const call = async (method: string, path: string, options: Call = {}): Promise<Reply> => {
    const headers = new Headers(options.headers);
    let body: string | undefined;
    if (options.raw !== undefined) {
      body = options.raw;
      headers.set("Content-Type", "application/json");
    } else if (options.body !== undefined) {
      body = JSON.stringify(options.body);
      headers.set("Content-Type", "application/json");
    }
    if (options.token) headers.set("Authorization", `Bearer ${options.token}`);
    const response = await app.fetch(new Request(`http://tracker.test${path}`, { method, headers, body }));
    const text = await response.text();
    return { status: response.status, body: text ? JSON.parse(text) : null, headers: response.headers };
  };
  return {
    app,
    call,
    /** Move the clock forward. */
    advance(ms: number) {
      now += ms;
    },
    /** The clock's time, as the API writes it. */
    iso() {
      return new Date(now).toISOString();
    },
  };
}

export type Api = ReturnType<typeof setup>;

/** A registered, signed-in user. */
export async function signUp(api: Api, name: string) {
  const email = `${name.toLowerCase()}@example.com`;
  const created = await api.call("POST", "/users", { body: { email, password: "correct horse", name } });
  if (created.status !== 201) throw new Error(`POST /users answered ${created.status}`);
  const session = await api.call("POST", "/sessions", { body: { email, password: "correct horse" } });
  if (session.status !== 201) throw new Error(`POST /sessions answered ${session.status}`);
  return { id: created.body.id as number, email, name, token: session.body.token as string };
}

export type User = Awaited<ReturnType<typeof signUp>>;

/** A project owned by `owner`. */
export async function project(api: Api, owner: User, key = "WEB", name = "Website") {
  const created = await api.call("POST", "/projects", { token: owner.token, body: { name, key } });
  if (created.status !== 201) throw new Error(`POST /projects answered ${created.status}`);
  return created.body as { id: number; key: string; name: string; ownerId: number; createdAt: string };
}

/** `user` added to the project as `role`. */
export async function join(api: Api, owner: User, projectId: number, user: User, role: "member" | "viewer") {
  const added = await api.call("POST", `/projects/${projectId}/members`, {
    token: owner.token,
    body: { userId: user.id, role },
  });
  if (added.status !== 201) throw new Error(`POST /projects/${projectId}/members answered ${added.status}`);
}

/** A task in the project, created by `user`. */
export async function task(api: Api, user: User, projectId: number, fields: Record<string, unknown> = {}) {
  const created = await api.call("POST", `/projects/${projectId}/tasks`, {
    token: user.token,
    body: { title: "A task", ...fields },
  });
  if (created.status !== 201) throw new Error(`POST /projects/${projectId}/tasks answered ${created.status}`);
  return created.body;
}

/** The error code of a reply. */
export function code(reply: Reply): string | undefined {
  return reply.body?.error?.code;
}
