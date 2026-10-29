// Hidden: versions, idempotency, limits and errors, on other data.
import { describe, expect, test } from "bun:test";
import { code, join, project, setup, signUp, task } from "../test/helpers";

async function pair() {
  const api = setup();
  const mia = await signUp(api, "Mia");
  const noa = await signUp(api, "Noa");
  const ops = await project(api, mia, "OPS", "Operations");
  await join(api, mia, ops.id, noa, "member");
  return { api, mia, noa, ops };
}

describe("hidden: versions", () => {
  test("a stale If-Match after a transition refuses a change", async () => {
    const { api, mia, noa, ops } = await pair();
    await task(api, mia, ops.id);
    await api.call("POST", "/tasks/OPS-1/transitions", { token: noa.token, body: { to: "in_progress" } });
    const reply = await api.call("PATCH", "/tasks/OPS-1", { token: mia.token, headers: { "If-Match": '"1"' }, body: { priority: "high" } });
    expect(reply.status).toBe(412);
    expect((await api.call("GET", "/tasks/OPS-1", { token: mia.token })).body.priority).toBe("normal");
  });

  test("the current version, read from the ETag, goes through", async () => {
    const { api, mia, ops } = await pair();
    await task(api, mia, ops.id);
    for (let n = 0; n < 3; n++) await api.call("PATCH", "/tasks/OPS-1", { token: mia.token, body: { title: `Title ${n}` } });
    const read = await api.call("GET", "/tasks/OPS-1", { token: mia.token });
    const etag = read.headers.get("ETag") ?? "";
    expect(etag).toBe('"4"');
    const reply = await api.call("PATCH", "/tasks/OPS-1", { token: mia.token, headers: { "If-Match": etag }, body: { title: "Final" } });
    expect(reply.status).toBe(200);
    expect(reply.body.version).toBe(5);
  });

  test("a version that never was is refused", async () => {
    const { api, mia, ops } = await pair();
    await task(api, mia, ops.id);
    const reply = await api.call("PATCH", "/tasks/OPS-1", { token: mia.token, headers: { "If-Match": '"7"' }, body: { title: "x" } });
    expect(code(reply)).toBe("precondition_failed");
  });
});

describe("hidden: idempotency", () => {
  test("a replay does not use up a number", async () => {
    const { api, mia, ops } = await pair();
    const send = () => api.call("POST", `/projects/${ops.id}/tasks`, { token: mia.token, headers: { "Idempotency-Key": "abc" }, body: { title: "Once" } });
    await send();
    await send();
    await send();
    const next = await task(api, mia, ops.id);
    expect(next.key).toBe("OPS-2");
  });

  test("a replay answers the task as it was created, after it changed", async () => {
    const { api, mia, ops } = await pair();
    const send = () => api.call("POST", `/projects/${ops.id}/tasks`, { token: mia.token, headers: { "Idempotency-Key": "abc" }, body: { title: "Once" } });
    const first = await send();
    await api.call("PATCH", "/tasks/OPS-1", { token: mia.token, body: { title: "Changed" } });
    const again = await send();
    expect(again.body).toEqual(first.body);
  });

  test("two keys make two tasks", async () => {
    const { api, mia, ops } = await pair();
    const a = await api.call("POST", `/projects/${ops.id}/tasks`, { token: mia.token, headers: { "Idempotency-Key": "a" }, body: { title: "Same" } });
    const b = await api.call("POST", `/projects/${ops.id}/tasks`, { token: mia.token, headers: { "Idempotency-Key": "b" }, body: { title: "Same" } });
    expect([a.body.key, b.body.key]).toEqual(["OPS-1", "OPS-2"]);
  });

  test("a mismatch is checked within the day only", async () => {
    const { api, mia, ops } = await pair();
    await api.call("POST", `/projects/${ops.id}/tasks`, { token: mia.token, headers: { "Idempotency-Key": "k" }, body: { title: "One" } });
    api.advance(24 * 60 * 60 * 1000);
    const later = await api.call("POST", `/projects/${ops.id}/tasks`, { token: mia.token, headers: { "Idempotency-Key": "k" }, body: { title: "Two" } });
    expect(later.status).toBe(201);
    expect(later.body.title).toBe("Two");
  });
});

describe("hidden: limits", () => {
  test("every authenticated route counts, writes too", async () => {
    const { api, mia, ops } = await pair();
    // pair() spent 2 of Mia's requests: the project and the join.
    for (let n = 0; n < 97; n++) await api.call("GET", `/projects/${ops.id}`, { token: mia.token });
    const last = await api.call("POST", `/projects/${ops.id}/tasks`, { token: mia.token, body: { title: "x" } });
    expect(last.status).toBe(201);
    expect(last.headers.get("X-RateLimit-Remaining")).toBe("0");
    const over = await api.call("POST", `/projects/${ops.id}/tasks`, { token: mia.token, body: { title: "y" } });
    expect(over.status).toBe(429);
    expect(over.headers.get("Retry-After")).toBe("60");
  });

  test("a refused request leaves no task behind", async () => {
    const { api, mia, noa, ops } = await pair();
    for (let n = 0; n < 98; n++) await api.call("GET", "/me", { token: mia.token });
    await api.call("POST", `/projects/${ops.id}/tasks`, { token: mia.token, body: { title: "over" } });
    const list = await api.call("GET", `/projects/${ops.id}/tasks`, { token: noa.token });
    expect(list.body.items).toHaveLength(0);
  });

  test("two sign-ins of one user are two tokens, two limits", async () => {
    const api = setup();
    const mia = await signUp(api, "Mia");
    const other = await api.call("POST", "/sessions", { body: { email: mia.email, password: "correct horse" } });
    for (let n = 0; n < 100; n++) await api.call("GET", "/me", { token: mia.token });
    expect((await api.call("GET", "/me", { token: other.body.token })).status).toBe(200);
  });
});

describe("hidden: errors", () => {
  test("a method /me does not take", async () => {
    const api = setup();
    const reply = await api.call("POST", "/me");
    expect(reply.status).toBe(405);
    expect(reply.headers.get("Allow")).toBe("GET");
  });

  test("a task key of no project is not found", async () => {
    const { api, mia } = await pair();
    expect((await api.call("GET", "/tasks/NOPE-1", { token: mia.token })).status).toBe(404);
    expect((await api.call("GET", "/tasks/garbage", { token: mia.token })).status).toBe(404);
  });

  test("a transition without a body, or with a broken one, is refused", async () => {
    const { api, mia, ops } = await pair();
    await task(api, mia, ops.id);
    const empty = await api.call("POST", "/tasks/OPS-1/transitions", { token: mia.token, body: {} });
    const broken = await api.call("POST", "/tasks/OPS-1/transitions", { token: mia.token, raw: "{to:" });
    expect([empty.status, broken.status]).toEqual([400, 400]);
    expect(empty.body.error.field).toBe("to");
  });

  test("an error answer is JSON with nothing beside the error", async () => {
    const { api, mia, ops } = await pair();
    const reply = await api.call("POST", `/projects/${ops.id}/members`, { token: mia.token, body: { userId: 99, role: "member" } });
    expect(reply.headers.get("Content-Type")).toStartWith("application/json");
    expect(Object.keys(reply.body)).toEqual(["error"]);
  });

  test("a 204 has no body", async () => {
    const { api, mia, ops } = await pair();
    await task(api, mia, ops.id);
    const reply = await api.call("DELETE", "/tasks/OPS-1", { token: mia.token });
    expect(reply.status).toBe(204);
    expect(reply.body).toBeNull();
  });
});
