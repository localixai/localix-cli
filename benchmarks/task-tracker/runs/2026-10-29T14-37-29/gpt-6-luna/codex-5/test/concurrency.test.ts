import { describe, expect, test } from "bun:test";
import { code, join, project, setup, signUp, task } from "./helpers";

async function shared() {
  const api = setup();
  const ada = await signUp(api, "Ada");
  const bob = await signUp(api, "Bob");
  const web = await project(api, ada);
  await join(api, ada, web.id, bob, "member");
  return { api, ada, bob, web };
}

describe("versions", () => {
  test("a task's ETag is its version, quoted", async () => {
    const { api, ada, web } = await shared();
    await task(api, ada, web.id);
    const read = await api.call("GET", "/tasks/WEB-1", { token: ada.token });
    expect(read.headers.get("ETag")).toBe('"1"');
    const changed = await api.call("PATCH", "/tasks/WEB-1", { token: ada.token, body: { title: "Renamed" } });
    expect(changed.headers.get("ETag")).toBe('"2"');
    const moved = await api.call("POST", "/tasks/WEB-1/transitions", { token: ada.token, body: { to: "in_progress" } });
    expect(moved.headers.get("ETag")).toBe('"3"');
  });

  test("a creation answers the new task's ETag", async () => {
    const { api, ada, web } = await shared();
    const created = await api.call("POST", `/projects/${web.id}/tasks`, { token: ada.token, body: { title: "A task" } });
    expect(created.headers.get("ETag")).toBe('"1"');
  });

  test("a change made against the version read goes through", async () => {
    const { api, ada, web } = await shared();
    await task(api, ada, web.id);
    const reply = await api.call("PATCH", "/tasks/WEB-1", {
      token: ada.token,
      headers: { "If-Match": '"1"' },
      body: { title: "Renamed" },
    });
    expect(reply.status).toBe(200);
  });

  test("of two people who read version 1, the second to write is told it changed", async () => {
    const { api, ada, bob, web } = await shared();
    await task(api, ada, web.id);
    const first = await api.call("PATCH", "/tasks/WEB-1", {
      token: ada.token,
      headers: { "If-Match": '"1"' },
      body: { title: "Ada's title" },
    });
    const second = await api.call("PATCH", "/tasks/WEB-1", {
      token: bob.token,
      headers: { "If-Match": '"1"' },
      body: { title: "Bob's title" },
    });
    expect([first.status, second.status]).toEqual([200, 412]);
    expect(code(second)).toBe("precondition_failed");
    expect((await api.call("GET", "/tasks/WEB-1", { token: ada.token })).body.title).toBe("Ada's title");
  });

  test("a transition checks If-Match too", async () => {
    const { api, ada, web } = await shared();
    await task(api, ada, web.id);
    await api.call("PATCH", "/tasks/WEB-1", { token: ada.token, body: { title: "Renamed" } });
    const reply = await api.call("POST", "/tasks/WEB-1/transitions", {
      token: ada.token,
      headers: { "If-Match": '"1"' },
      body: { to: "in_progress" },
    });
    expect(reply.status).toBe(412);
    expect((await api.call("GET", "/tasks/WEB-1", { token: ada.token })).body.status).toBe("open");
  });
});

describe("idempotency", () => {
  const create = (token: string, key: string, title = "Pay the invoice") => ({
    token,
    headers: { "Idempotency-Key": key },
    body: { title },
  });

  test("a creation sent twice with one key makes one task, and the same answer", async () => {
    const { api, ada, web } = await shared();
    const first = await api.call("POST", `/projects/${web.id}/tasks`, create(ada.token, "k-1"));
    api.advance(1000);
    const again = await api.call("POST", `/projects/${web.id}/tasks`, create(ada.token, "k-1"));
    expect([first.status, again.status]).toEqual([201, 201]);
    expect(again.body).toEqual(first.body);
    expect(again.headers.get("Idempotent-Replayed")).toBe("true");
    expect(first.headers.get("Idempotent-Replayed")).toBeNull();
    const list = await api.call("GET", `/projects/${web.id}/tasks`, { token: ada.token });
    expect(list.body.items).toHaveLength(1);
  });

  test("one key with another body is refused", async () => {
    const { api, ada, web } = await shared();
    await api.call("POST", `/projects/${web.id}/tasks`, create(ada.token, "k-1"));
    const other = await api.call("POST", `/projects/${web.id}/tasks`, create(ada.token, "k-1", "Another title"));
    expect(other.status).toBe(422);
    expect(code(other)).toBe("idempotency_mismatch");
  });

  test("keys are each user's own", async () => {
    const { api, ada, bob, web } = await shared();
    const mine = await api.call("POST", `/projects/${web.id}/tasks`, create(ada.token, "k-1"));
    const theirs = await api.call("POST", `/projects/${web.id}/tasks`, create(bob.token, "k-1"));
    expect([mine.body.key, theirs.body.key]).toEqual(["WEB-1", "WEB-2"]);
  });

  test("a key is kept for 24 hours", async () => {
    const { api, ada, web } = await shared();
    await api.call("POST", `/projects/${web.id}/tasks`, create(ada.token, "k-1"));
    api.advance(24 * 60 * 60 * 1000 - 1);
    const within = await api.call("POST", `/projects/${web.id}/tasks`, create(ada.token, "k-1"));
    api.advance(1);
    const after = await api.call("POST", `/projects/${web.id}/tasks`, create(ada.token, "k-1"));
    expect([within.body.key, after.body.key]).toEqual(["WEB-1", "WEB-2"]);
  });

  test("a refused creation is not kept: the same key works once the body is fixed", async () => {
    const { api, ada, web } = await shared();
    const refused = await api.call("POST", `/projects/${web.id}/tasks`, create(ada.token, "k-1", ""));
    const fixed = await api.call("POST", `/projects/${web.id}/tasks`, create(ada.token, "k-1"));
    expect([refused.status, fixed.status]).toEqual([400, 201]);
  });
});
