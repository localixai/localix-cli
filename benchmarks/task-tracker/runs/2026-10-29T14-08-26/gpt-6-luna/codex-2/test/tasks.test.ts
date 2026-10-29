import { describe, expect, test } from "bun:test";
import { code, join, project, setup, signUp, task } from "./helpers";

async function team() {
  const api = setup();
  const ada = await signUp(api, "Ada");
  const bob = await signUp(api, "Bob");
  const cid = await signUp(api, "Cid");
  const eve = await signUp(api, "Eve");
  const web = await project(api, ada);
  await join(api, ada, web.id, bob, "member");
  await join(api, ada, web.id, cid, "viewer");
  return { api, ada, bob, cid, eve, web };
}

describe("creating tasks", () => {
  test("a task takes its project's key and the next number", async () => {
    const { api, ada, web } = await team();
    const reply = await api.call("POST", `/projects/${web.id}/tasks`, {
      token: ada.token,
      body: { title: "  Fix the login page  " },
    });
    expect(reply.status).toBe(201);
    expect(reply.body).toEqual({
      id: 1,
      key: "WEB-1",
      number: 1,
      projectId: web.id,
      title: "Fix the login page",
      description: "",
      status: "open",
      priority: "normal",
      labels: [],
      assigneeId: null,
      createdBy: ada.id,
      createdAt: api.iso(),
      updatedAt: api.iso(),
      version: 1,
    });
  });

  test("numbers count per project", async () => {
    const { api, ada, web } = await team();
    const ops = await project(api, ada, "OPS", "Operations");
    const first = await task(api, ada, web.id);
    const second = await task(api, ada, web.id);
    const other = await task(api, ada, ops.id);
    expect([first.key, second.key, other.key]).toEqual(["WEB-1", "WEB-2", "OPS-1"]);
    expect([first.id, second.id, other.id]).toEqual([1, 2, 3]);
  });

  test("labels are trimmed, put in lower case, made unique and sorted", async () => {
    const { api, ada, web } = await team();
    const created = await task(api, ada, web.id, { labels: ["UI", " bug ", "ui", "a11y"] });
    expect(created.labels).toEqual(["a11y", "bug", "ui"]);
  });

  test("a task takes a description, a priority and an assignee from the project", async () => {
    const { api, ada, bob, web } = await team();
    const created = await task(api, ada, web.id, { description: "Steps: open /login", priority: "high", assigneeId: bob.id });
    expect(created).toMatchObject({ description: "Steps: open /login", priority: "high", assigneeId: bob.id });
  });

  test.each([
    ["an empty title", { title: "  " }, "title"],
    ["a title over 200 characters", { title: "t".repeat(201) }, "title"],
    ["a description that is not a string", { title: "A task", description: 5 }, "description"],
    ["a priority that is not low, normal or high", { title: "A task", priority: "urgent" }, "priority"],
    ["labels that are not a list", { title: "A task", labels: "bug" }, "labels"],
    ["a label with a space inside", { title: "A task", labels: ["needs review"] }, "labels"],
    ["a label over 30 characters", { title: "A task", labels: ["l".repeat(31)] }, "labels"],
  ])("%s is refused", async (_, body, field) => {
    const { api, ada, web } = await team();
    const reply = await api.call("POST", `/projects/${web.id}/tasks`, { token: ada.token, body });
    expect(reply.status).toBe(400);
    expect(reply.body.error).toMatchObject({ code: "validation", field });
  });

  test("an assignee from outside the project is refused", async () => {
    const { api, ada, eve, web } = await team();
    const reply = await api.call("POST", `/projects/${web.id}/tasks`, {
      token: ada.token,
      body: { title: "A task", assigneeId: eve.id },
    });
    expect(reply.status).toBe(400);
    expect(reply.body.error).toMatchObject({ code: "validation", field: "assigneeId" });
  });

  test("a member creates tasks; a viewer may not; for anyone else the project is not found", async () => {
    const { api, bob, cid, eve, web } = await team();
    const member = await api.call("POST", `/projects/${web.id}/tasks`, { token: bob.token, body: { title: "A task" } });
    const viewer = await api.call("POST", `/projects/${web.id}/tasks`, { token: cid.token, body: { title: "A task" } });
    const stranger = await api.call("POST", `/projects/${web.id}/tasks`, { token: eve.token, body: { title: "A task" } });
    expect([member.status, viewer.status, stranger.status]).toEqual([201, 403, 404]);
    expect(code(viewer)).toBe("forbidden");
  });

  test("permission is checked before the body: a viewer's empty title is still forbidden", async () => {
    const { api, cid, web } = await team();
    const reply = await api.call("POST", `/projects/${web.id}/tasks`, { token: cid.token, body: { title: "" } });
    expect(reply.status).toBe(403);
  });
});

describe("reading, changing and deleting a task", () => {
  test("a task is read by its key, by anyone in the project", async () => {
    const { api, ada, cid, eve, web } = await team();
    const created = await task(api, ada, web.id);
    const viewer = await api.call("GET", "/tasks/WEB-1", { token: cid.token });
    const stranger = await api.call("GET", "/tasks/WEB-1", { token: eve.token });
    const missing = await api.call("GET", "/tasks/WEB-9", { token: ada.token });
    expect(viewer.status).toBe(200);
    expect(viewer.body).toEqual(created);
    expect([stranger.status, missing.status]).toEqual([404, 404]);
  });

  test("a change answers the task with a new version and update time", async () => {
    const { api, ada, bob, web } = await team();
    const created = await task(api, ada, web.id);
    api.advance(5000);
    const reply = await api.call("PATCH", "/tasks/WEB-1", {
      token: bob.token,
      body: { title: "Fix the sign-in page", priority: "low", labels: ["Auth"], assigneeId: bob.id },
    });
    expect(reply.status).toBe(200);
    expect(reply.body).toEqual({
      ...created,
      title: "Fix the sign-in page",
      priority: "low",
      labels: ["auth"],
      assigneeId: bob.id,
      updatedAt: api.iso(),
      version: 2,
    });
    expect((await api.call("GET", "/tasks/WEB-1", { token: ada.token })).body).toEqual(reply.body);
  });

  test("an assignee of null unassigns the task", async () => {
    const { api, ada, bob, web } = await team();
    await task(api, ada, web.id, { assigneeId: bob.id });
    const reply = await api.call("PATCH", "/tasks/WEB-1", { token: ada.token, body: { assigneeId: null } });
    expect(reply.body.assigneeId).toBeNull();
  });

  test("a change names no field it does not know, nor the status", async () => {
    const { api, ada, web } = await team();
    await task(api, ada, web.id);
    const unknown = await api.call("PATCH", "/tasks/WEB-1", { token: ada.token, body: { colour: "red" } });
    const status = await api.call("PATCH", "/tasks/WEB-1", { token: ada.token, body: { status: "done" } });
    expect(unknown.status).toBe(400);
    expect(unknown.body.error).toMatchObject({ code: "validation", field: "colour" });
    expect(status.status).toBe(400);
    expect(status.body.error).toMatchObject({ code: "validation", field: "status" });
  });

  test("a viewer may not change a task", async () => {
    const { api, ada, cid, web } = await team();
    await task(api, ada, web.id);
    const reply = await api.call("PATCH", "/tasks/WEB-1", { token: cid.token, body: { title: "Mine now" } });
    expect(reply.status).toBe(403);
  });

  test("a member deletes their own tasks, the owner any; a number is never given again", async () => {
    const { api, ada, bob, web } = await team();
    await task(api, ada, web.id);
    await task(api, bob, web.id);
    const others = await api.call("DELETE", "/tasks/WEB-1", { token: bob.token });
    const own = await api.call("DELETE", "/tasks/WEB-2", { token: bob.token });
    const owner = await api.call("DELETE", "/tasks/WEB-1", { token: ada.token });
    expect([others.status, own.status, owner.status]).toEqual([403, 204, 204]);
    expect((await api.call("GET", "/tasks/WEB-2", { token: ada.token })).status).toBe(404);
    const next = await task(api, ada, web.id);
    expect(next.key).toBe("WEB-3");
  });

  test("removing a member unassigns their tasks", async () => {
    const { api, ada, bob, web } = await team();
    await task(api, ada, web.id, { assigneeId: bob.id });
    await api.call("DELETE", `/projects/${web.id}/members/${bob.id}`, { token: ada.token });
    const reply = await api.call("GET", "/tasks/WEB-1", { token: ada.token });
    expect(reply.body.assigneeId).toBeNull();
  });
});

describe("comments", () => {
  test("anyone in the project comments, a viewer too, oldest first", async () => {
    const { api, ada, cid, web } = await team();
    await task(api, ada, web.id);
    const first = await api.call("POST", "/tasks/WEB-1/comments", { token: cid.token, body: { body: "Seen on Safari" } });
    api.advance(1000);
    await api.call("POST", "/tasks/WEB-1/comments", { token: ada.token, body: { body: "Thanks" } });
    expect(first.status).toBe(201);
    expect(first.body).toEqual({ id: 1, authorId: cid.id, body: "Seen on Safari", createdAt: "2026-01-01T00:00:00.000Z" });
    const list = await api.call("GET", "/tasks/WEB-1/comments", { token: ada.token });
    expect(list.status).toBe(200);
    expect(list.body.map((c: { body: string }) => c.body)).toEqual(["Seen on Safari", "Thanks"]);
  });

  test("a comment does not change the task's version", async () => {
    const { api, ada, web } = await team();
    await task(api, ada, web.id);
    await api.call("POST", "/tasks/WEB-1/comments", { token: ada.token, body: { body: "Noted" } });
    expect((await api.call("GET", "/tasks/WEB-1", { token: ada.token })).body.version).toBe(1);
  });

  test("an empty comment, or one over 2000 characters, is refused", async () => {
    const { api, ada, web } = await team();
    await task(api, ada, web.id);
    const empty = await api.call("POST", "/tasks/WEB-1/comments", { token: ada.token, body: { body: " " } });
    const long = await api.call("POST", "/tasks/WEB-1/comments", { token: ada.token, body: { body: "c".repeat(2001) } });
    expect([empty.status, long.status]).toEqual([400, 400]);
    expect(empty.body.error).toMatchObject({ code: "validation", field: "body" });
  });
});
