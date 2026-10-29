// Hidden: dependencies, batches and stats, on other data.
import { describe, expect, test } from "bun:test";
import { code, join, project, setup, signUp, task } from "../test/helpers";

async function plant() {
  const api = setup();
  const mia = await signUp(api, "Mia");
  const noa = await signUp(api, "Noa");
  const ole = await signUp(api, "Ole");
  const ops = await project(api, mia, "OPS", "Operations");
  await join(api, mia, ops.id, noa, "member");
  await join(api, mia, ops.id, ole, "viewer");
  for (let n = 0; n < 5; n++) await task(api, mia, ops.id, { title: `Step ${n + 1}` });
  return { api, mia, noa, ole, ops };
}

const on = (token: string, dependsOn: string) => ({ token, body: { dependsOn } });
const move = (token: string, to: string) => ({ token, body: { to } });

describe("hidden: dependencies", () => {
  test("a task with two dependencies waits for both", async () => {
    const { api, mia } = await plant();
    await api.call("POST", "/tasks/OPS-5/dependencies", on(mia.token, "OPS-3"));
    await api.call("POST", "/tasks/OPS-5/dependencies", on(mia.token, "OPS-1"));
    for (const to of ["in_progress", "done"]) await api.call("POST", "/tasks/OPS-1/transitions", move(mia.token, to));
    await api.call("POST", "/tasks/OPS-5/transitions", move(mia.token, "in_progress"));
    expect(code(await api.call("POST", "/tasks/OPS-5/transitions", move(mia.token, "done")))).toBe("blocked");
    for (const to of ["in_progress", "done"]) await api.call("POST", "/tasks/OPS-3/transitions", move(mia.token, to));
    expect((await api.call("POST", "/tasks/OPS-5/transitions", move(mia.token, "done"))).status).toBe(200);
  });

  test("a reopened dependency blocks again", async () => {
    const { api, mia } = await plant();
    await api.call("POST", "/tasks/OPS-2/dependencies", on(mia.token, "OPS-1"));
    for (const to of ["in_progress", "done", "open"]) await api.call("POST", "/tasks/OPS-1/transitions", move(mia.token, to));
    await api.call("POST", "/tasks/OPS-2/transitions", move(mia.token, "in_progress"));
    expect(code(await api.call("POST", "/tasks/OPS-2/transitions", move(mia.token, "done")))).toBe("blocked");
  });

  test("a cycle through four tasks is refused, a diamond is not", async () => {
    const { api, mia } = await plant();
    for (const [from, to] of [["OPS-2", "OPS-1"], ["OPS-3", "OPS-1"], ["OPS-4", "OPS-2"], ["OPS-4", "OPS-3"]]) {
      expect((await api.call("POST", `/tasks/${from}/dependencies`, on(mia.token, to!))).status).toBe(201);
    }
    const cycle = await api.call("POST", "/tasks/OPS-1/dependencies", on(mia.token, "OPS-4"));
    expect(code(cycle)).toBe("dependency_cycle");
    expect((await api.call("POST", "/tasks/OPS-5/dependencies", on(mia.token, "OPS-4"))).status).toBe(201);
  });

  test("a refused cycle adds nothing", async () => {
    const { api, mia } = await plant();
    await api.call("POST", "/tasks/OPS-2/dependencies", on(mia.token, "OPS-1"));
    await api.call("POST", "/tasks/OPS-1/dependencies", on(mia.token, "OPS-2"));
    const one = await api.call("GET", "/tasks/OPS-1/dependencies", { token: mia.token });
    expect(one.body).toEqual({ key: "OPS-1", dependsOn: [], blocks: ["OPS-2"] });
  });

  test("a stranger reaches no dependency; a viewer removes none", async () => {
    const { api, mia, ole } = await plant();
    const pia = await signUp(api, "Pia");
    await api.call("POST", "/tasks/OPS-2/dependencies", on(mia.token, "OPS-1"));
    expect((await api.call("GET", "/tasks/OPS-2/dependencies", { token: pia.token })).status).toBe(404);
    expect((await api.call("DELETE", "/tasks/OPS-2/dependencies/OPS-1", { token: ole.token })).status).toBe(403);
  });

  test("a dependency named by something that is not a string is refused", async () => {
    const { api, mia } = await plant();
    const reply = await api.call("POST", "/tasks/OPS-2/dependencies", { token: mia.token, body: { dependsOn: 1 } });
    expect(reply.body.error).toMatchObject({ code: "validation", field: "dependsOn" });
  });
});

describe("hidden: batches", () => {
  test("an assignee from outside refuses a batch at its place", async () => {
    const { api, mia, ops } = await plant();
    const pia = await signUp(api, "Pia");
    const reply = await api.call("POST", `/projects/${ops.id}/tasks/bulk`, {
      token: mia.token,
      body: { tasks: [{ title: "a" }, { title: "b", labels: ["ok"] }, { title: "c" }, { title: "d", assigneeId: pia.id }] },
    });
    expect(reply.body.error).toMatchObject({ code: "validation", field: "tasks[3].assigneeId" });
    expect((await api.call("GET", `/projects/${ops.id}/stats`, { token: mia.token })).body.total).toBe(5);
  });

  test("a bad label is named by its task's place", async () => {
    const { api, mia, ops } = await plant();
    const reply = await api.call("POST", `/projects/${ops.id}/tasks/bulk`, {
      token: mia.token,
      body: { tasks: [{ title: "a", labels: ["two words"] }] },
    });
    expect(reply.body.error.field).toBe("tasks[0].labels");
  });

  test("a batch goes on from the project's last number, and its tasks have no history", async () => {
    const { api, noa, ops } = await plant();
    const reply = await api.call("POST", `/projects/${ops.id}/tasks/bulk`, { token: noa.token, body: { tasks: [{ title: "x" }] } });
    expect(reply.body.items[0]).toMatchObject({ key: "OPS-6", id: 6, createdBy: noa.id });
    expect((await api.call("GET", "/tasks/OPS-6/history", { token: noa.token })).body).toEqual([]);
  });

  test("a batch that is not a list is refused", async () => {
    const { api, mia, ops } = await plant();
    const reply = await api.call("POST", `/projects/${ops.id}/tasks/bulk`, { token: mia.token, body: { tasks: { title: "x" } } });
    expect(reply.body.error).toMatchObject({ code: "validation", field: "tasks" });
  });
});

describe("hidden: stats", () => {
  test("a project with no tasks counts zero everywhere", async () => {
    const api = setup();
    const mia = await signUp(api, "Mia");
    const p = await project(api, mia, "MIA");
    const reply = await api.call("GET", `/projects/${p.id}/stats`, { token: mia.token });
    expect(reply.body).toEqual({
      total: 0,
      byStatus: { open: 0, in_progress: 0, done: 0 },
      byPriority: { high: 0, normal: 0, low: 0 },
      byAssignee: [],
    });
  });

  test("a tie in assignees goes by user number", async () => {
    const { api, mia, noa, ops } = await plant();
    await api.call("PATCH", "/tasks/OPS-1", { token: mia.token, body: { assigneeId: noa.id } });
    await api.call("PATCH", "/tasks/OPS-2", { token: mia.token, body: { assigneeId: mia.id } });
    const reply = await api.call("GET", `/projects/${ops.id}/stats`, { token: mia.token });
    expect(reply.body.byAssignee).toEqual([
      { userId: mia.id, count: 1 },
      { userId: noa.id, count: 1 },
    ]);
  });

  test("done tasks are counted as done", async () => {
    const { api, mia, ops } = await plant();
    for (const key of ["OPS-1", "OPS-2"]) {
      for (const to of ["in_progress", "done"]) await api.call("POST", `/tasks/${key}/transitions`, move(mia.token, to));
    }
    const reply = await api.call("GET", `/projects/${ops.id}/stats`, { token: mia.token });
    expect(reply.body.byStatus).toEqual({ open: 3, in_progress: 0, done: 2 });
    expect(reply.body.byPriority).toEqual({ high: 0, normal: 5, low: 0 });
  });
});
