// Hidden: the rules the public tests state, on other data. Copied into the run's folder as
// hidden/, beside test/, after the agent has finished.
import { describe, expect, test } from "bun:test";
import { code, join, project, setup, signUp, task } from "../test/helpers";

describe("hidden: accounts", () => {
  test("a password of exactly 8 characters and a name of exactly 100 are taken", async () => {
    const api = setup();
    const reply = await api.call("POST", "/users", {
      body: { email: "zed@corp.io", password: "12345678", name: "z".repeat(100) },
    });
    expect(reply.status).toBe(201);
    expect(reply.body.name).toHaveLength(100);
  });

  test("a third user is number 3, whatever their email", async () => {
    const api = setup();
    await signUp(api, "Mia");
    await signUp(api, "Noa");
    const reply = await api.call("POST", "/users", { body: { email: "OLE@Host.Org", password: "long enough", name: "Ole" } });
    expect(reply.body).toEqual({ id: 3, email: "ole@host.org", name: "Ole" });
  });

  test("a password that is not a string is refused", async () => {
    const api = setup();
    const reply = await api.call("POST", "/users", { body: { email: "mia@example.com", password: 12345678, name: "Mia" } });
    expect(reply.status).toBe(400);
    expect(reply.body.error).toMatchObject({ code: "validation", field: "password" });
  });

  test("signing in with the right password of another user fails", async () => {
    const api = setup();
    await signUp(api, "Mia");
    await api.call("POST", "/users", { body: { email: "noa@example.com", password: "noa's secret", name: "Noa" } });
    const reply = await api.call("POST", "/sessions", { body: { email: "noa@example.com", password: "correct horse" } });
    expect(reply.status).toBe(401);
  });

  test("a token signed out cannot sign out again", async () => {
    const api = setup();
    const mia = await signUp(api, "Mia");
    await api.call("DELETE", "/sessions", { token: mia.token });
    const again = await api.call("DELETE", "/sessions", { token: mia.token });
    expect(again.status).toBe(401);
  });

  test("a signed-out token reaches no project", async () => {
    const api = setup();
    const mia = await signUp(api, "Mia");
    const p = await project(api, mia, "MIA", "Mia's");
    await api.call("DELETE", "/sessions", { token: mia.token });
    expect((await api.call("GET", `/projects/${p.id}`, { token: mia.token })).status).toBe(401);
  });
});

describe("hidden: projects and members", () => {
  test("keys of 2 and of 6 letters are taken", async () => {
    const api = setup();
    const mia = await signUp(api, "Mia");
    const two = await api.call("POST", "/projects", { token: mia.token, body: { name: "Ops", key: "OP" } });
    const six = await api.call("POST", "/projects", { token: mia.token, body: { name: "Mobile", key: "MOBILE" } });
    expect([two.status, six.status]).toEqual([201, 201]);
    expect([two.body.id, six.body.id]).toEqual([1, 2]);
  });

  test("a project's creation time is the clock's", async () => {
    const api = setup();
    const mia = await signUp(api, "Mia");
    api.advance(3_600_000);
    const p = await project(api, mia, "DOCS", "Docs");
    expect(p.createdAt).toBe("2026-01-01T01:00:00.000Z");
  });

  test("a user in no project lists none", async () => {
    const api = setup();
    const mia = await signUp(api, "Mia");
    const noa = await signUp(api, "Noa");
    await project(api, mia, "MIA");
    const reply = await api.call("GET", "/projects", { token: noa.token });
    expect(reply.status).toBe(200);
    expect(reply.body).toEqual([]);
  });

  test("a stranger's member list is not found; a viewer reads it", async () => {
    const api = setup();
    const mia = await signUp(api, "Mia");
    const noa = await signUp(api, "Noa");
    const ole = await signUp(api, "Ole");
    const p = await project(api, mia, "MIA");
    await join(api, mia, p.id, noa, "viewer");
    const viewer = await api.call("GET", `/projects/${p.id}/members`, { token: noa.token });
    const stranger = await api.call("GET", `/projects/${p.id}/members`, { token: ole.token });
    expect(viewer.body).toEqual([
      { userId: mia.id, role: "owner" },
      { userId: noa.id, role: "viewer" },
    ]);
    expect(stranger.status).toBe(404);
  });

  test("members are listed by user number, whatever order they joined in", async () => {
    const api = setup();
    const mia = await signUp(api, "Mia");
    const noa = await signUp(api, "Noa");
    const ole = await signUp(api, "Ole");
    const p = await project(api, ole, "OLE");
    await join(api, ole, p.id, noa, "member");
    await join(api, ole, p.id, mia, "viewer");
    const list = await api.call("GET", `/projects/${p.id}/members`, { token: ole.token });
    expect(list.body).toEqual([
      { userId: ole.id, role: "owner" },
      { userId: mia.id, role: "viewer" },
      { userId: noa.id, role: "member" },
    ]);
  });

  test("only the owner removes members", async () => {
    const api = setup();
    const mia = await signUp(api, "Mia");
    const noa = await signUp(api, "Noa");
    const ole = await signUp(api, "Ole");
    const p = await project(api, mia, "MIA");
    await join(api, mia, p.id, noa, "member");
    await join(api, mia, p.id, ole, "member");
    const reply = await api.call("DELETE", `/projects/${p.id}/members/${ole.id}`, { token: noa.token });
    expect(reply.status).toBe(403);
  });

  test("a stranger adding members finds no project", async () => {
    const api = setup();
    const mia = await signUp(api, "Mia");
    const noa = await signUp(api, "Noa");
    const p = await project(api, mia, "MIA");
    const reply = await api.call("POST", `/projects/${p.id}/members`, { token: noa.token, body: { userId: noa.id, role: "member" } });
    expect(reply.status).toBe(404);
  });

  test("a removed member's tasks stay, unassigned, and the member can join again", async () => {
    const api = setup();
    const mia = await signUp(api, "Mia");
    const noa = await signUp(api, "Noa");
    const p = await project(api, mia, "MIA");
    await join(api, mia, p.id, noa, "member");
    await task(api, noa, p.id, { title: "Noa's own", assigneeId: noa.id });
    await api.call("DELETE", `/projects/${p.id}/members/${noa.id}`, { token: mia.token });
    const left = await api.call("GET", "/tasks/MIA-1", { token: mia.token });
    expect(left.body).toMatchObject({ createdBy: noa.id, assigneeId: null });
    const back = await api.call("POST", `/projects/${p.id}/members`, { token: mia.token, body: { userId: noa.id, role: "viewer" } });
    expect(back.status).toBe(201);
    expect(code(await api.call("POST", `/projects/${p.id}/tasks`, { token: noa.token, body: { title: "x" } }))).toBe("forbidden");
  });
});
