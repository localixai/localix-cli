import { describe, expect, test } from "bun:test";
import { code, join, project, setup, signUp } from "./helpers";

describe("projects", () => {
  test("a project is created by its owner", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const reply = await api.call("POST", "/projects", { token: ada.token, body: { name: " Website ", key: "WEB" } });
    expect(reply.status).toBe(201);
    expect(reply.body).toEqual({ id: 1, name: "Website", key: "WEB", ownerId: ada.id, createdAt: api.iso() });
  });

  test("creating a project needs a token", async () => {
    const api = setup();
    const reply = await api.call("POST", "/projects", { body: { name: "Website", key: "WEB" } });
    expect(reply.status).toBe(401);
  });

  test.each([
    ["a key in lower case", { name: "Website", key: "web" }, "key"],
    ["a key of one letter", { name: "Website", key: "W" }, "key"],
    ["a key of seven letters", { name: "Website", key: "WEBSITE" }, "key"],
    ["a key with a digit", { name: "Website", key: "WEB2" }, "key"],
    ["an empty name", { name: "", key: "WEB" }, "name"],
    ["a name over 100 characters", { name: "n".repeat(101), key: "WEB" }, "name"],
  ])("%s is refused", async (_, body, field) => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const reply = await api.call("POST", "/projects", { token: ada.token, body });
    expect(reply.status).toBe(400);
    expect(reply.body.error).toMatchObject({ code: "validation", field });
  });

  test("a key is unique across every owner", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const bob = await signUp(api, "Bob");
    await project(api, ada, "WEB");
    const reply = await api.call("POST", "/projects", { token: bob.token, body: { name: "Web again", key: "WEB" } });
    expect(reply.status).toBe(409);
    expect(reply.body.error).toMatchObject({ code: "conflict", field: "key" });
  });

  test("a user lists the projects they own or belong to, oldest first", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const bob = await signUp(api, "Bob");
    const web = await project(api, ada, "WEB");
    api.advance(1000);
    const app = await project(api, bob, "APP", "Mobile app");
    api.advance(1000);
    const ops = await project(api, ada, "OPS", "Operations");
    await join(api, bob, app.id, ada, "viewer");
    const ofAda = await api.call("GET", "/projects", { token: ada.token });
    const ofBob = await api.call("GET", "/projects", { token: bob.token });
    expect(ofAda.status).toBe(200);
    expect(ofAda.body.map((p: { key: string }) => p.key)).toEqual(["WEB", "APP", "OPS"]);
    expect(ofBob.body.map((p: { key: string }) => p.key)).toEqual(["APP"]);
    expect(ofAda.body[0]).toEqual(web);
    expect(ofAda.body[2]).toEqual(ops);
  });

  test("a project is read by its members, and is not found for anyone else", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const bob = await signUp(api, "Bob");
    const web = await project(api, ada);
    const own = await api.call("GET", `/projects/${web.id}`, { token: ada.token });
    const stranger = await api.call("GET", `/projects/${web.id}`, { token: bob.token });
    const missing = await api.call("GET", "/projects/99", { token: ada.token });
    expect(own.status).toBe(200);
    expect(own.body).toEqual(web);
    expect([stranger.status, missing.status]).toEqual([404, 404]);
    expect(code(stranger)).toBe("not_found");
  });

  test("a project id that is not a number is not found", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const reply = await api.call("GET", "/projects/web", { token: ada.token });
    expect(reply.status).toBe(404);
  });
});

describe("members", () => {
  test("the owner adds a member, and the member lists come owner first", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const bob = await signUp(api, "Bob");
    const cid = await signUp(api, "Cid");
    const web = await project(api, ada);
    const added = await api.call("POST", `/projects/${web.id}/members`, {
      token: ada.token,
      body: { userId: cid.id, role: "viewer" },
    });
    expect(added.status).toBe(201);
    expect(added.body).toEqual({ userId: cid.id, role: "viewer" });
    await join(api, ada, web.id, bob, "member");
    const list = await api.call("GET", `/projects/${web.id}/members`, { token: bob.token });
    expect(list.status).toBe(200);
    expect(list.body).toEqual([
      { userId: ada.id, role: "owner" },
      { userId: bob.id, role: "member" },
      { userId: cid.id, role: "viewer" },
    ]);
  });

  test("only the owner adds members", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const bob = await signUp(api, "Bob");
    const cid = await signUp(api, "Cid");
    const web = await project(api, ada);
    await join(api, ada, web.id, bob, "member");
    const reply = await api.call("POST", `/projects/${web.id}/members`, {
      token: bob.token,
      body: { userId: cid.id, role: "member" },
    });
    expect(reply.status).toBe(403);
    expect(code(reply)).toBe("forbidden");
  });

  test("a role that is not member or viewer, or a user who does not exist, is refused", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const bob = await signUp(api, "Bob");
    const web = await project(api, ada);
    const role = await api.call("POST", `/projects/${web.id}/members`, {
      token: ada.token,
      body: { userId: bob.id, role: "admin" },
    });
    const nobody = await api.call("POST", `/projects/${web.id}/members`, {
      token: ada.token,
      body: { userId: 99, role: "member" },
    });
    expect(role.status).toBe(400);
    expect(role.body.error).toMatchObject({ code: "validation", field: "role" });
    expect(nobody.status).toBe(400);
    expect(nobody.body.error).toMatchObject({ code: "validation", field: "userId" });
  });

  test("adding someone already in the project, the owner too, is a conflict", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const bob = await signUp(api, "Bob");
    const web = await project(api, ada);
    await join(api, ada, web.id, bob, "member");
    const twice = await api.call("POST", `/projects/${web.id}/members`, {
      token: ada.token,
      body: { userId: bob.id, role: "viewer" },
    });
    const owner = await api.call("POST", `/projects/${web.id}/members`, {
      token: ada.token,
      body: { userId: ada.id, role: "member" },
    });
    expect([twice.status, owner.status]).toEqual([409, 409]);
    expect(code(twice)).toBe("conflict");
  });

  test("a removed member no longer finds the project", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const bob = await signUp(api, "Bob");
    const web = await project(api, ada);
    await join(api, ada, web.id, bob, "member");
    const removed = await api.call("DELETE", `/projects/${web.id}/members/${bob.id}`, { token: ada.token });
    expect(removed.status).toBe(204);
    expect((await api.call("GET", `/projects/${web.id}`, { token: bob.token })).status).toBe(404);
    const again = await api.call("DELETE", `/projects/${web.id}/members/${bob.id}`, { token: ada.token });
    expect(again.status).toBe(404);
  });
});
