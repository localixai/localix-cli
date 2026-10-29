// Hidden: tasks, lists, status and history, on other data.
import { describe, expect, test } from "bun:test";
import { code, join, project, setup, signUp, task, type Api, type User } from "../test/helpers";

async function crew() {
  const api = setup();
  const mia = await signUp(api, "Mia");
  const noa = await signUp(api, "Noa");
  const ole = await signUp(api, "Ole");
  const ops = await project(api, mia, "OPS", "Operations");
  await join(api, mia, ops.id, noa, "member");
  await join(api, mia, ops.id, ole, "viewer");
  return { api, mia, noa, ole, ops };
}

async function keys(api: Api, user: User, path: string) {
  const reply = await api.call("GET", path, { token: user.token });
  if (reply.status !== 200) throw new Error(`GET ${path} answered ${reply.status}`);
  return reply.body.items.map((t: { key: string }) => t.key);
}

describe("hidden: tasks", () => {
  test("a title of exactly 200 characters and a label of exactly 30 are taken", async () => {
    const { api, mia, ops } = await crew();
    const created = await task(api, mia, ops.id, { title: "t".repeat(200), labels: ["l".repeat(30)] });
    expect(created.title).toHaveLength(200);
    expect(created.labels).toEqual(["l".repeat(30)]);
  });

  test("a member's task has the member as creator, and the creation time", async () => {
    const { api, noa, ops } = await crew();
    api.advance(90_000);
    const created = await task(api, noa, ops.id, { title: "Rotate the keys", priority: "low" });
    expect(created).toMatchObject({ key: "OPS-1", createdBy: noa.id, createdAt: "2026-01-01T00:01:30.000Z", priority: "low" });
  });

  test("tasks are numbered on, and ids go on across projects", async () => {
    const { api, mia, ops } = await crew();
    const sec = await project(api, mia, "SEC", "Security");
    await task(api, mia, sec.id);
    await task(api, mia, ops.id);
    const third = await task(api, mia, sec.id);
    expect([third.key, third.number, third.id]).toEqual(["SEC-2", 2, 3]);
  });

  test("a label made of digits and dashes is taken; an empty one is refused", async () => {
    const { api, mia, ops } = await crew();
    const ok = await task(api, mia, ops.id, { labels: ["q3-2026"] });
    expect(ok.labels).toEqual(["q3-2026"]);
    const empty = await api.call("POST", `/projects/${ops.id}/tasks`, { token: mia.token, body: { title: "x", labels: ["  "] } });
    expect(empty.status).toBe(400);
    expect(empty.body.error.field).toBe("labels");
  });

  test("the owner and a member may be assignees", async () => {
    const { api, mia, noa, ops } = await crew();
    const created = await task(api, mia, ops.id, { assigneeId: mia.id });
    expect(created.assigneeId).toBe(mia.id);
    const changed = await api.call("PATCH", "/tasks/OPS-1", { token: mia.token, body: { assigneeId: noa.id } });
    expect(changed.body.assigneeId).toBe(noa.id);
  });

  test("a change to an assignee from outside is refused", async () => {
    const { api, mia, ops } = await crew();
    const outsider = await signUp(api, "Pia");
    await task(api, mia, ops.id);
    const reply = await api.call("PATCH", "/tasks/OPS-1", { token: mia.token, body: { assigneeId: outsider.id } });
    expect(reply.status).toBe(400);
    expect(reply.body.error.field).toBe("assigneeId");
  });

  test("a change with an empty title is refused and changes nothing", async () => {
    const { api, mia, ops } = await crew();
    await task(api, mia, ops.id, { title: "Keep me" });
    const reply = await api.call("PATCH", "/tasks/OPS-1", { token: mia.token, body: { title: " ", priority: "high" } });
    expect(reply.status).toBe(400);
    const read = await api.call("GET", "/tasks/OPS-1", { token: mia.token });
    expect(read.body).toMatchObject({ title: "Keep me", priority: "normal", version: 1 });
  });

  test("a stranger's change or deletion finds no task", async () => {
    const { api, mia, ops } = await crew();
    const pia = await signUp(api, "Pia");
    await task(api, mia, ops.id);
    const patch = await api.call("PATCH", "/tasks/OPS-1", { token: pia.token, body: { title: "x" } });
    const del = await api.call("DELETE", "/tasks/OPS-1", { token: pia.token });
    expect([patch.status, del.status]).toEqual([404, 404]);
  });

  test("a viewer deletes nothing, not even with a comment of theirs on it", async () => {
    const { api, mia, ole, ops } = await crew();
    await task(api, mia, ops.id);
    await api.call("POST", "/tasks/OPS-1/comments", { token: ole.token, body: { body: "mine" } });
    expect((await api.call("DELETE", "/tasks/OPS-1", { token: ole.token })).status).toBe(403);
  });

  test("a deleted task's comments and history are gone with it", async () => {
    const { api, mia, ops } = await crew();
    await task(api, mia, ops.id);
    await api.call("DELETE", "/tasks/OPS-1", { token: mia.token });
    expect((await api.call("GET", "/tasks/OPS-1/comments", { token: mia.token })).status).toBe(404);
    expect((await api.call("GET", "/tasks/OPS-1/history", { token: mia.token })).status).toBe(404);
  });

  test("comments are numbered across tasks, and a stranger reads none", async () => {
    const { api, mia, noa, ops } = await crew();
    const pia = await signUp(api, "Pia");
    await task(api, mia, ops.id);
    await task(api, mia, ops.id);
    await api.call("POST", "/tasks/OPS-1/comments", { token: mia.token, body: { body: "one" } });
    const second = await api.call("POST", "/tasks/OPS-2/comments", { token: noa.token, body: { body: "two" } });
    expect(second.body).toMatchObject({ id: 2, authorId: noa.id, body: "two" });
    expect((await api.call("GET", "/tasks/OPS-2/comments", { token: pia.token })).status).toBe(404);
  });
});

describe("hidden: lists", () => {
  async function filled() {
    const c = await crew();
    const { api, mia, noa, ops } = c;
    const specs = [
      { title: "Renew the certificate", priority: "high", labels: ["infra"] },
      { title: "Page the on-call", assigneeId: noa.id, labels: ["infra", "alerts"] },
      { title: "Tidy the dashboards", priority: "low", description: "Remove the old certificate panel" },
      { title: "Backup restore drill", priority: "high", assigneeId: noa.id },
      { title: "Alert on disk usage", labels: ["alerts"] },
      { title: "Archive the logs", priority: "low", assigneeId: mia.id, labels: ["infra"] },
      { title: "Patch the kernel", priority: "high", labels: ["infra", "security"] },
    ];
    for (const spec of specs) {
      await task(api, mia, ops.id, spec);
      api.advance(500);
    }
    return c;
  }

  test("priority order on other tasks", async () => {
    const { api, mia, ops } = await filled();
    expect(await keys(api, mia, `/projects/${ops.id}/tasks?sort=priority`)).toEqual(["OPS-1", "OPS-4", "OPS-7", "OPS-2", "OPS-5", "OPS-3", "OPS-6"]);
  });

  test("a label and newest first", async () => {
    const { api, mia, ops } = await filled();
    expect(await keys(api, mia, `/projects/${ops.id}/tasks?label=infra&sort=-created`)).toEqual(["OPS-7", "OPS-6", "OPS-2", "OPS-1"]);
  });

  test("a search reaches the description, and none matches nothing", async () => {
    const { api, mia, ops } = await filled();
    expect(await keys(api, mia, `/projects/${ops.id}/tasks?q=CERTIFICATE`)).toEqual(["OPS-1", "OPS-3"]);
    const none = await api.call("GET", `/projects/${ops.id}/tasks?q=zebra`, { token: mia.token });
    expect(none.body).toEqual({ items: [], nextCursor: null });
  });

  test("one task a page, every page, to the end", async () => {
    const { api, mia, noa, ops } = await filled();
    const seen: string[] = [];
    let cursor: string | null = null;
    let pages = 0;
    do {
      const path: string = `/projects/${ops.id}/tasks?assignee=${noa.id}&sort=-created&limit=1${cursor ? `&cursor=${encodeURIComponent(cursor)}` : ""}`;
      const reply = await api.call("GET", path, { token: mia.token });
      seen.push(...reply.body.items.map((t: { key: string }) => t.key));
      cursor = reply.body.nextCursor;
      pages++;
    } while (cursor && pages < 10);
    expect(seen).toEqual(["OPS-4", "OPS-2"]);
    expect(pages).toBe(2);
  });

  test("three pages by priority, nothing seen twice", async () => {
    const { api, mia, ops } = await filled();
    const seen: string[] = [];
    let cursor: string | null = null;
    for (let page = 0; page < 3; page++) {
      const path: string = `/projects/${ops.id}/tasks?sort=priority&limit=3${cursor ? `&cursor=${encodeURIComponent(cursor)}` : ""}`;
      const reply = await api.call("GET", path, { token: mia.token });
      seen.push(...reply.body.items.map((t: { key: string }) => t.key));
      cursor = reply.body.nextCursor;
    }
    expect(seen).toEqual(["OPS-1", "OPS-4", "OPS-7", "OPS-2", "OPS-5", "OPS-3", "OPS-6"]);
    expect(cursor).toBeNull();
  });

  test("a limit of 100 is taken", async () => {
    const { api, mia, ops } = await filled();
    const reply = await api.call("GET", `/projects/${ops.id}/tasks?limit=100`, { token: mia.token });
    expect(reply.status).toBe(200);
    expect(reply.body.items).toHaveLength(7);
  });

  test("a deleted task leaves the list", async () => {
    const { api, mia, ops } = await filled();
    await api.call("DELETE", "/tasks/OPS-4", { token: mia.token });
    expect(await keys(api, mia, `/projects/${ops.id}/tasks?sort=priority`)).toEqual(["OPS-1", "OPS-7", "OPS-2", "OPS-5", "OPS-3", "OPS-6"]);
  });

  test("a viewer lists; a stranger finds no project", async () => {
    const { api, ole, ops } = await filled();
    const pia = await signUp(api, "Pia");
    expect((await api.call("GET", `/projects/${ops.id}/tasks`, { token: ole.token })).status).toBe(200);
    expect((await api.call("GET", `/projects/${ops.id}/tasks`, { token: pia.token })).status).toBe(404);
  });

  test("status done after a whole round", async () => {
    const { api, mia, ops } = await filled();
    for (const to of ["in_progress", "done"]) await api.call("POST", "/tasks/OPS-5/transitions", { token: mia.token, body: { to } });
    expect(await keys(api, mia, `/projects/${ops.id}/tasks?status=done`)).toEqual(["OPS-5"]);
  });
});

describe("hidden: status and history", () => {
  test("a reopened task's history tells the whole round", async () => {
    const { api, mia, noa, ops } = await crew();
    await task(api, mia, ops.id);
    for (const [user, to] of [[noa, "in_progress"], [noa, "done"], [mia, "open"]] as const) {
      api.advance(1000);
      await api.call("POST", "/tasks/OPS-1/transitions", { token: user.token, body: { to } });
    }
    const history = await api.call("GET", "/tasks/OPS-1/history", { token: mia.token });
    expect(history.body).toEqual([
      { at: "2026-01-01T00:00:01.000Z", by: noa.id, field: "status", from: "open", to: "in_progress" },
      { at: "2026-01-01T00:00:02.000Z", by: noa.id, field: "status", from: "in_progress", to: "done" },
      { at: "2026-01-01T00:00:03.000Z", by: mia.id, field: "status", from: "done", to: "open" },
    ]);
  });

  test("a description change is an entry; a refused transition is none", async () => {
    const { api, mia, ops } = await crew();
    await task(api, mia, ops.id, { description: "old" });
    await api.call("PATCH", "/tasks/OPS-1", { token: mia.token, body: { description: "new" } });
    const refused = await api.call("POST", "/tasks/OPS-1/transitions", { token: mia.token, body: { to: "done" } });
    expect(code(refused)).toBe("invalid_transition");
    const history = await api.call("GET", "/tasks/OPS-1/history", { token: mia.token });
    expect(history.body.map((e: { field: string; from: string; to: string }) => [e.field, e.from, e.to])).toEqual([["description", "old", "new"]]);
  });

  test("labels given in another order and case are no change", async () => {
    const { api, mia, ops } = await crew();
    await task(api, mia, ops.id, { labels: ["infra", "alerts"] });
    const reply = await api.call("PATCH", "/tasks/OPS-1", { token: mia.token, body: { labels: ["Alerts", "INFRA"] } });
    expect(reply.body.version).toBe(1);
    expect((await api.call("GET", "/tasks/OPS-1/history", { token: mia.token })).body).toEqual([]);
  });

  test("an unassignment is an entry to null", async () => {
    const { api, mia, noa, ops } = await crew();
    await task(api, mia, ops.id, { assigneeId: noa.id });
    await api.call("PATCH", "/tasks/OPS-1", { token: mia.token, body: { assigneeId: null } });
    const history = await api.call("GET", "/tasks/OPS-1/history", { token: mia.token });
    expect(history.body).toEqual([{ at: "2026-01-01T00:00:00.000Z", by: mia.id, field: "assigneeId", from: noa.id, to: null }]);
  });
});
