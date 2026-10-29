import { describe, expect, test } from "bun:test";
import { code, join, project, setup, signUp, task } from "./helpers";

async function chain() {
  const api = setup();
  const ada = await signUp(api, "Ada");
  const bob = await signUp(api, "Bob");
  const cid = await signUp(api, "Cid");
  const web = await project(api, ada);
  await join(api, ada, web.id, bob, "member");
  await join(api, ada, web.id, cid, "viewer");
  for (const title of ["Design the schema", "Write the migration", "Ship the release", "Announce it"]) {
    await task(api, ada, web.id, { title });
  }
  return { api, ada, bob, cid, web };
}

const depend = (token: string, on: string) => ({ token, body: { dependsOn: on } });

describe("dependencies", () => {
  test("a task depends on another, and the other blocks it", async () => {
    const { api, ada } = await chain();
    const added = await api.call("POST", "/tasks/WEB-2/dependencies", depend(ada.token, "WEB-1"));
    expect(added.status).toBe(201);
    expect(added.body).toEqual({ key: "WEB-2", dependsOn: ["WEB-1"], blocks: [] });
    const other = await api.call("GET", "/tasks/WEB-1/dependencies", { token: ada.token });
    expect(other.status).toBe(200);
    expect(other.body).toEqual({ key: "WEB-1", dependsOn: [], blocks: ["WEB-2"] });
  });

  test("both lists are in number order", async () => {
    const { api, ada } = await chain();
    await api.call("POST", "/tasks/WEB-3/dependencies", depend(ada.token, "WEB-2"));
    await api.call("POST", "/tasks/WEB-3/dependencies", depend(ada.token, "WEB-1"));
    await api.call("POST", "/tasks/WEB-4/dependencies", depend(ada.token, "WEB-1"));
    const three = await api.call("GET", "/tasks/WEB-3/dependencies", { token: ada.token });
    const one = await api.call("GET", "/tasks/WEB-1/dependencies", { token: ada.token });
    expect(three.body.dependsOn).toEqual(["WEB-1", "WEB-2"]);
    expect(one.body.blocks).toEqual(["WEB-3", "WEB-4"]);
  });

  test("a task cannot depend on itself, on a task of another project, or on one that does not exist", async () => {
    const { api, ada } = await chain();
    const ops = await project(api, ada, "OPS", "Operations");
    await task(api, ada, ops.id);
    for (const on of ["WEB-2", "OPS-1", "WEB-9"]) {
      const reply = await api.call("POST", "/tasks/WEB-2/dependencies", depend(ada.token, on));
      expect(reply.status).toBe(400);
      expect(reply.body.error).toMatchObject({ code: "validation", field: "dependsOn" });
    }
  });

  test("a dependency given twice is a conflict", async () => {
    const { api, ada } = await chain();
    await api.call("POST", "/tasks/WEB-2/dependencies", depend(ada.token, "WEB-1"));
    const again = await api.call("POST", "/tasks/WEB-2/dependencies", depend(ada.token, "WEB-1"));
    expect(again.status).toBe(409);
    expect(code(again)).toBe("conflict");
  });

  test("a dependency that would close a cycle is refused, however long the cycle", async () => {
    const { api, ada } = await chain();
    await api.call("POST", "/tasks/WEB-2/dependencies", depend(ada.token, "WEB-1"));
    await api.call("POST", "/tasks/WEB-3/dependencies", depend(ada.token, "WEB-2"));
    const direct = await api.call("POST", "/tasks/WEB-1/dependencies", depend(ada.token, "WEB-2"));
    const long = await api.call("POST", "/tasks/WEB-1/dependencies", depend(ada.token, "WEB-3"));
    expect([direct.status, long.status]).toEqual([409, 409]);
    expect([code(direct), code(long)]).toEqual(["dependency_cycle", "dependency_cycle"]);
  });

  test("a task is not done while a task it depends on is not", async () => {
    const { api, ada } = await chain();
    await api.call("POST", "/tasks/WEB-2/dependencies", depend(ada.token, "WEB-1"));
    await api.call("POST", "/tasks/WEB-2/transitions", { token: ada.token, body: { to: "in_progress" } });
    const blocked = await api.call("POST", "/tasks/WEB-2/transitions", { token: ada.token, body: { to: "done" } });
    expect(blocked.status).toBe(409);
    expect(code(blocked)).toBe("blocked");
    for (const to of ["in_progress", "done"]) {
      await api.call("POST", "/tasks/WEB-1/transitions", { token: ada.token, body: { to } });
    }
    const done = await api.call("POST", "/tasks/WEB-2/transitions", { token: ada.token, body: { to: "done" } });
    expect(done.status).toBe(200);
  });

  test("a dependency is removed, and a removed one is not found", async () => {
    const { api, ada } = await chain();
    await api.call("POST", "/tasks/WEB-2/dependencies", depend(ada.token, "WEB-1"));
    const removed = await api.call("DELETE", "/tasks/WEB-2/dependencies/WEB-1", { token: ada.token });
    expect(removed.status).toBe(204);
    expect((await api.call("GET", "/tasks/WEB-2/dependencies", { token: ada.token })).body.dependsOn).toEqual([]);
    const again = await api.call("DELETE", "/tasks/WEB-2/dependencies/WEB-1", { token: ada.token });
    expect(again.status).toBe(404);
  });

  test("a deleted task leaves every dependency", async () => {
    const { api, ada } = await chain();
    await api.call("POST", "/tasks/WEB-2/dependencies", depend(ada.token, "WEB-1"));
    await api.call("POST", "/tasks/WEB-1/dependencies", depend(ada.token, "WEB-4"));
    await api.call("DELETE", "/tasks/WEB-1", { token: ada.token });
    expect((await api.call("GET", "/tasks/WEB-2/dependencies", { token: ada.token })).body.dependsOn).toEqual([]);
    expect((await api.call("GET", "/tasks/WEB-4/dependencies", { token: ada.token })).body.blocks).toEqual([]);
  });

  test("a member adds dependencies; a viewer reads them but adds none", async () => {
    const { api, bob, cid } = await chain();
    const member = await api.call("POST", "/tasks/WEB-2/dependencies", depend(bob.token, "WEB-1"));
    const viewer = await api.call("POST", "/tasks/WEB-3/dependencies", depend(cid.token, "WEB-1"));
    const read = await api.call("GET", "/tasks/WEB-2/dependencies", { token: cid.token });
    expect([member.status, viewer.status, read.status]).toEqual([201, 403, 200]);
  });

  test("a dependency changes neither task's version", async () => {
    const { api, ada } = await chain();
    await api.call("POST", "/tasks/WEB-2/dependencies", depend(ada.token, "WEB-1"));
    const read = await api.call("GET", "/tasks/WEB-2", { token: ada.token });
    expect(read.body.version).toBe(1);
  });
});
