import { describe, expect, test } from "bun:test";
import { join, project, setup, signUp, task, type Api, type User } from "./helpers";

async function board() {
  const api = setup();
  const ada = await signUp(api, "Ada");
  const bob = await signUp(api, "Bob");
  const web = await project(api, ada);
  await join(api, ada, web.id, bob, "member");
  // WEB-1 .. WEB-6, a second apart.
  const specs = [
    { title: "Login button is grey", priority: "high", labels: ["ui", "bug"], assigneeId: bob.id },
    { title: "Write the onboarding guide", description: "Cover the LOGIN flow too", priority: "low" },
    { title: "Cache the avatars", labels: ["perf"], assigneeId: ada.id },
    { title: "Dark mode", priority: "high", labels: ["ui"] },
    { title: "Crash on empty cart", labels: ["bug"], assigneeId: bob.id },
    { title: "Rename the settings tab", priority: "low", labels: ["ui"] },
  ];
  for (const spec of specs) {
    await task(api, ada, web.id, spec);
    api.advance(1000);
  }
  return { api, ada, bob, web };
}

async function keys(api: Api, user: User, path: string) {
  const reply = await api.call("GET", path, { token: user.token });
  if (reply.status !== 200) throw new Error(`GET ${path} answered ${reply.status}`);
  return reply.body.items.map((t: { key: string }) => t.key);
}

describe("listing tasks", () => {
  test("a list is the project's tasks, oldest first, with no next page", async () => {
    const { api, ada, web } = await board();
    const reply = await api.call("GET", `/projects/${web.id}/tasks`, { token: ada.token });
    expect(reply.status).toBe(200);
    expect(reply.body.items.map((t: { key: string }) => t.key)).toEqual(["WEB-1", "WEB-2", "WEB-3", "WEB-4", "WEB-5", "WEB-6"]);
    expect(reply.body.items[0].title).toBe("Login button is grey");
    expect(reply.body.nextCursor).toBeNull();
  });

  test("filters by status", async () => {
    const { api, ada, web } = await board();
    await api.call("POST", "/tasks/WEB-2/transitions", { token: ada.token, body: { to: "in_progress" } });
    expect(await keys(api, ada, `/projects/${web.id}/tasks?status=in_progress`)).toEqual(["WEB-2"]);
    expect(await keys(api, ada, `/projects/${web.id}/tasks?status=open`)).toEqual(["WEB-1", "WEB-3", "WEB-4", "WEB-5", "WEB-6"]);
  });

  test("filters by assignee: an id, me, or none", async () => {
    const { api, ada, bob, web } = await board();
    expect(await keys(api, ada, `/projects/${web.id}/tasks?assignee=${bob.id}`)).toEqual(["WEB-1", "WEB-5"]);
    expect(await keys(api, ada, `/projects/${web.id}/tasks?assignee=me`)).toEqual(["WEB-3"]);
    expect(await keys(api, bob, `/projects/${web.id}/tasks?assignee=me`)).toEqual(["WEB-1", "WEB-5"]);
    expect(await keys(api, ada, `/projects/${web.id}/tasks?assignee=none`)).toEqual(["WEB-2", "WEB-4", "WEB-6"]);
  });

  test("filters by label, in any case", async () => {
    const { api, ada, web } = await board();
    expect(await keys(api, ada, `/projects/${web.id}/tasks?label=UI`)).toEqual(["WEB-1", "WEB-4", "WEB-6"]);
  });

  test("searches the title and the description, in any case", async () => {
    const { api, ada, web } = await board();
    expect(await keys(api, ada, `/projects/${web.id}/tasks?q=login`)).toEqual(["WEB-1", "WEB-2"]);
    expect(await keys(api, ada, `/projects/${web.id}/tasks?q=${encodeURIComponent("empty cart")}`)).toEqual(["WEB-5"]);
  });

  test("filters combine", async () => {
    const { api, ada, bob, web } = await board();
    expect(await keys(api, ada, `/projects/${web.id}/tasks?label=bug&assignee=${bob.id}&q=crash`)).toEqual(["WEB-5"]);
  });

  test("sorts newest first, or by priority, high first, then by number", async () => {
    const { api, ada, web } = await board();
    expect(await keys(api, ada, `/projects/${web.id}/tasks?sort=-created`)).toEqual(["WEB-6", "WEB-5", "WEB-4", "WEB-3", "WEB-2", "WEB-1"]);
    expect(await keys(api, ada, `/projects/${web.id}/tasks?sort=priority`)).toEqual(["WEB-1", "WEB-4", "WEB-3", "WEB-5", "WEB-2", "WEB-6"]);
  });

  test("pages by a cursor, as far as the last page", async () => {
    const { api, ada, web } = await board();
    const first = await api.call("GET", `/projects/${web.id}/tasks?limit=4`, { token: ada.token });
    expect(first.body.items.map((t: { key: string }) => t.key)).toEqual(["WEB-1", "WEB-2", "WEB-3", "WEB-4"]);
    expect(typeof first.body.nextCursor).toBe("string");
    const cursor = encodeURIComponent(first.body.nextCursor);
    const second = await api.call("GET", `/projects/${web.id}/tasks?limit=4&cursor=${cursor}`, { token: ada.token });
    expect(second.body.items.map((t: { key: string }) => t.key)).toEqual(["WEB-5", "WEB-6"]);
    expect(second.body.nextCursor).toBeNull();
  });

  test("a cursor keeps its sort and filters' order", async () => {
    const { api, ada, web } = await board();
    const path = `/projects/${web.id}/tasks?sort=priority&limit=2`;
    const first = await api.call("GET", path, { token: ada.token });
    const cursor = encodeURIComponent(first.body.nextCursor);
    const second = await api.call("GET", `${path}&cursor=${cursor}`, { token: ada.token });
    expect(second.body.items.map((t: { key: string }) => t.key)).toEqual(["WEB-3", "WEB-5"]);
  });

  test("a page that ends exactly at the last task has no next page", async () => {
    const { api, ada, web } = await board();
    const reply = await api.call("GET", `/projects/${web.id}/tasks?limit=6`, { token: ada.token });
    expect(reply.body.items).toHaveLength(6);
    expect(reply.body.nextCursor).toBeNull();
  });

  test.each([
    ["status=closed", "status"],
    ["sort=title", "sort"],
    ["limit=0", "limit"],
    ["limit=101", "limit"],
    ["limit=ten", "limit"],
    ["cursor=garbage", "cursor"],
    ["assignee=someone", "assignee"],
  ])("%s is refused", async (query, field) => {
    const { api, ada, web } = await board();
    const reply = await api.call("GET", `/projects/${web.id}/tasks?${query}`, { token: ada.token });
    expect(reply.status).toBe(400);
    expect(reply.body.error).toMatchObject({ code: "validation", field });
  });

  test("a list holds 20 tasks unless a limit says otherwise", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const web = await project(api, ada);
    for (let n = 0; n < 25; n++) await task(api, ada, web.id);
    const reply = await api.call("GET", `/projects/${web.id}/tasks`, { token: ada.token });
    expect(reply.body.items).toHaveLength(20);
    expect(reply.body.items[19].key).toBe("WEB-20");
  });
});
