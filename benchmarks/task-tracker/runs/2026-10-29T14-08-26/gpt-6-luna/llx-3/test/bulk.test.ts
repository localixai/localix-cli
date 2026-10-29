import { describe, expect, test } from "bun:test";
import { code, join, project, setup, signUp, task } from "./helpers";

async function office() {
  const api = setup();
  const ada = await signUp(api, "Ada");
  const bob = await signUp(api, "Bob");
  const cid = await signUp(api, "Cid");
  const web = await project(api, ada);
  await join(api, ada, web.id, bob, "member");
  await join(api, ada, web.id, cid, "viewer");
  return { api, ada, bob, cid, web };
}

describe("creating tasks in bulk", () => {
  test("a batch makes its tasks in order, each as one creation would", async () => {
    const { api, ada, bob, web } = await office();
    await task(api, ada, web.id, { title: "Already there" });
    const reply = await api.call("POST", `/projects/${web.id}/tasks/bulk`, {
      token: ada.token,
      body: { tasks: [{ title: "First" }, { title: "Second", priority: "high", labels: ["UI"], assigneeId: bob.id }] },
    });
    expect(reply.status).toBe(201);
    expect(reply.body.items.map((t: { key: string }) => t.key)).toEqual(["WEB-2", "WEB-3"]);
    expect(reply.body.items[1]).toMatchObject({ title: "Second", priority: "high", labels: ["ui"], assigneeId: bob.id, version: 1 });
    expect((await api.call("GET", "/tasks/WEB-3", { token: ada.token })).body).toEqual(reply.body.items[1]);
  });

  test("one bad task refuses the whole batch, naming it by its place", async () => {
    const { api, ada, web } = await office();
    const reply = await api.call("POST", `/projects/${web.id}/tasks/bulk`, {
      token: ada.token,
      body: { tasks: [{ title: "Fine" }, { title: "Also fine" }, { title: "" }] },
    });
    expect(reply.status).toBe(400);
    expect(reply.body.error).toMatchObject({ code: "validation", field: "tasks[2].title" });
    const list = await api.call("GET", `/projects/${web.id}/tasks`, { token: ada.token });
    expect(list.body.items).toHaveLength(0);
    expect((await task(api, ada, web.id)).key).toBe("WEB-1");
  });

  test("the first bad task is the one named", async () => {
    const { api, ada, web } = await office();
    const reply = await api.call("POST", `/projects/${web.id}/tasks/bulk`, {
      token: ada.token,
      body: { tasks: [{ title: "Fine" }, { title: "Fine", priority: "urgent" }, { title: "" }] },
    });
    expect(reply.body.error.field).toBe("tasks[1].priority");
  });

  test("a batch is a list of 1 to 50 tasks", async () => {
    const { api, ada, web } = await office();
    const empty = await api.call("POST", `/projects/${web.id}/tasks/bulk`, { token: ada.token, body: { tasks: [] } });
    const many = await api.call("POST", `/projects/${web.id}/tasks/bulk`, {
      token: ada.token,
      body: { tasks: Array.from({ length: 51 }, (_, n) => ({ title: `Task ${n}` })) },
    });
    const none = await api.call("POST", `/projects/${web.id}/tasks/bulk`, { token: ada.token, body: {} });
    for (const reply of [empty, many, none]) {
      expect(reply.status).toBe(400);
      expect(reply.body.error).toMatchObject({ code: "validation", field: "tasks" });
    }
  });

  test("a batch of 50 is taken", async () => {
    const { api, ada, web } = await office();
    const reply = await api.call("POST", `/projects/${web.id}/tasks/bulk`, {
      token: ada.token,
      body: { tasks: Array.from({ length: 50 }, (_, n) => ({ title: `Task ${n}` })) },
    });
    expect(reply.status).toBe(201);
    expect(reply.body.items[49].key).toBe("WEB-50");
  });

  test("a viewer makes no batch", async () => {
    const { api, cid, web } = await office();
    const reply = await api.call("POST", `/projects/${web.id}/tasks/bulk`, { token: cid.token, body: { tasks: [{ title: "x" }] } });
    expect(reply.status).toBe(403);
    expect(code(reply)).toBe("forbidden");
  });
});

describe("project stats", () => {
  test("counts by status and by priority, every bucket named", async () => {
    const { api, ada, cid, web } = await office();
    await task(api, ada, web.id, { priority: "high" });
    await task(api, ada, web.id, { priority: "high" });
    await task(api, ada, web.id, { priority: "low" });
    await api.call("POST", "/tasks/WEB-1/transitions", { token: ada.token, body: { to: "in_progress" } });
    const reply = await api.call("GET", `/projects/${web.id}/stats`, { token: cid.token });
    expect(reply.status).toBe(200);
    expect(reply.body).toEqual({
      total: 3,
      byStatus: { open: 2, in_progress: 1, done: 0 },
      byPriority: { high: 2, normal: 0, low: 1 },
      byAssignee: [],
    });
  });

  test("counts by assignee, unassigned left out, by count then user number", async () => {
    const { api, ada, bob, web } = await office();
    await task(api, ada, web.id, { assigneeId: ada.id });
    await task(api, ada, web.id, { assigneeId: bob.id });
    await task(api, ada, web.id, { assigneeId: bob.id });
    await task(api, ada, web.id);
    const reply = await api.call("GET", `/projects/${web.id}/stats`, { token: ada.token });
    expect(reply.body.byAssignee).toEqual([
      { userId: bob.id, count: 2 },
      { userId: ada.id, count: 1 },
    ]);
  });

  test("a deleted task is not counted", async () => {
    const { api, ada, web } = await office();
    await task(api, ada, web.id);
    await task(api, ada, web.id);
    await api.call("DELETE", "/tasks/WEB-1", { token: ada.token });
    const reply = await api.call("GET", `/projects/${web.id}/stats`, { token: ada.token });
    expect(reply.body.total).toBe(1);
  });

  test("a stranger finds no stats", async () => {
    const { api, web } = await office();
    const eve = await signUp(api, "Eve");
    expect((await api.call("GET", `/projects/${web.id}/stats`, { token: eve.token })).status).toBe(404);
  });
});
