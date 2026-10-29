import { describe, expect, test } from "bun:test";
import { code, join, project, setup, signUp, task } from "./helpers";

async function started() {
  const api = setup();
  const ada = await signUp(api, "Ada");
  const bob = await signUp(api, "Bob");
  const cid = await signUp(api, "Cid");
  const web = await project(api, ada);
  await join(api, ada, web.id, bob, "member");
  await join(api, ada, web.id, cid, "viewer");
  await task(api, ada, web.id, { title: "Fix the login page" });
  return { api, ada, bob, cid, web };
}

const move = (to: string) => ({ body: { to } });

describe("status", () => {
  test("a task goes open, in progress, done, and is reopened", async () => {
    const { api, ada } = await started();
    const steps = [];
    for (const to of ["in_progress", "done", "open"]) {
      const reply = await api.call("POST", "/tasks/WEB-1/transitions", { token: ada.token, ...move(to) });
      steps.push([reply.status, reply.body.status, reply.body.version]);
    }
    expect(steps).toEqual([
      [200, "in_progress", 2],
      [200, "done", 3],
      [200, "open", 4],
    ]);
  });

  test("work in progress can go back to open", async () => {
    const { api, ada } = await started();
    await api.call("POST", "/tasks/WEB-1/transitions", { token: ada.token, ...move("in_progress") });
    const reply = await api.call("POST", "/tasks/WEB-1/transitions", { token: ada.token, ...move("open") });
    expect(reply.status).toBe(200);
    expect(reply.body.status).toBe("open");
  });

  test.each([
    ["open", "done"],
    ["open", "open"],
  ])("from %s to %s is not a transition", async (_, to) => {
    const { api, ada } = await started();
    const reply = await api.call("POST", "/tasks/WEB-1/transitions", { token: ada.token, ...move(to) });
    expect(reply.status).toBe(409);
    expect(code(reply)).toBe("invalid_transition");
  });

  test("done cannot go to in progress, nor stay done", async () => {
    const { api, ada } = await started();
    await api.call("POST", "/tasks/WEB-1/transitions", { token: ada.token, ...move("in_progress") });
    await api.call("POST", "/tasks/WEB-1/transitions", { token: ada.token, ...move("done") });
    const back = await api.call("POST", "/tasks/WEB-1/transitions", { token: ada.token, ...move("in_progress") });
    const again = await api.call("POST", "/tasks/WEB-1/transitions", { token: ada.token, ...move("done") });
    expect([back.status, again.status]).toEqual([409, 409]);
  });

  test("a status that does not exist is refused", async () => {
    const { api, ada } = await started();
    const reply = await api.call("POST", "/tasks/WEB-1/transitions", { token: ada.token, ...move("closed") });
    expect(reply.status).toBe(400);
    expect(reply.body.error).toMatchObject({ code: "validation", field: "to" });
  });

  test("a member moves a task; a viewer may not", async () => {
    const { api, bob, cid } = await started();
    const member = await api.call("POST", "/tasks/WEB-1/transitions", { token: bob.token, ...move("in_progress") });
    const viewer = await api.call("POST", "/tasks/WEB-1/transitions", { token: cid.token, ...move("done") });
    expect([member.status, viewer.status]).toEqual([200, 403]);
  });
});

describe("history", () => {
  test("a new task has no history", async () => {
    const { api, ada } = await started();
    const reply = await api.call("GET", "/tasks/WEB-1/history", { token: ada.token });
    expect(reply.status).toBe(200);
    expect(reply.body).toEqual([]);
  });

  test("each change is an entry: who, when, the field, from and to", async () => {
    const { api, ada, bob } = await started();
    api.advance(60_000);
    await api.call("POST", "/tasks/WEB-1/transitions", { token: bob.token, ...move("in_progress") });
    const moved = api.iso();
    api.advance(60_000);
    await api.call("PATCH", "/tasks/WEB-1", { token: ada.token, body: { title: "Fix the sign-in page" } });
    const renamed = api.iso();
    const reply = await api.call("GET", "/tasks/WEB-1/history", { token: ada.token });
    expect(reply.body).toEqual([
      { at: moved, by: bob.id, field: "status", from: "open", to: "in_progress" },
      { at: renamed, by: ada.id, field: "title", from: "Fix the login page", to: "Fix the sign-in page" },
    ]);
  });

  test("a change of several fields is an entry each, in the order of their names", async () => {
    const { api, ada, bob } = await started();
    await api.call("PATCH", "/tasks/WEB-1", {
      token: ada.token,
      body: { title: "Fix the sign-in page", assigneeId: bob.id, labels: ["auth"], priority: "high" },
    });
    const reply = await api.call("GET", "/tasks/WEB-1/history", { token: ada.token });
    expect(reply.body.map((e: { field: string; from: unknown; to: unknown }) => [e.field, e.from, e.to])).toEqual([
      ["assigneeId", null, bob.id],
      ["labels", [], ["auth"]],
      ["priority", "normal", "high"],
      ["title", "Fix the login page", "Fix the sign-in page"],
    ]);
  });

  test("a field set to what it was makes no entry, and no new version", async () => {
    const { api, ada } = await started();
    const reply = await api.call("PATCH", "/tasks/WEB-1", { token: ada.token, body: { title: "Fix the login page", priority: "normal" } });
    expect(reply.status).toBe(200);
    expect(reply.body.version).toBe(1);
    expect((await api.call("GET", "/tasks/WEB-1/history", { token: ada.token })).body).toEqual([]);
  });

  test("a viewer reads the history", async () => {
    const { api, cid } = await started();
    expect((await api.call("GET", "/tasks/WEB-1/history", { token: cid.token })).status).toBe(200);
  });
});
