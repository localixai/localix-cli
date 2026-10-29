import { describe, expect, test } from "bun:test";
import { code, project, setup, signUp } from "./helpers";

describe("errors", () => {
  test("every error is one shape: a code and a message, and the field when one is to blame", async () => {
    const api = setup();
    const reply = await api.call("POST", "/users", { body: { email: "ada@example.com", password: "x", name: "Ada" } });
    expect(Object.keys(reply.body)).toEqual(["error"]);
    expect(Object.keys(reply.body.error).sort()).toEqual(["code", "field", "message"]);
    const unauthorized = await api.call("GET", "/me");
    expect(Object.keys(unauthorized.body.error).sort()).toEqual(["code", "message"]);
  });

  test("answers are JSON", async () => {
    const api = setup();
    const ok = await api.call("POST", "/users", { body: { email: "ada@example.com", password: "correct horse", name: "Ada" } });
    const error = await api.call("GET", "/me");
    expect(ok.headers.get("Content-Type")).toStartWith("application/json");
    expect(error.headers.get("Content-Type")).toStartWith("application/json");
  });

  test("a path that does not exist is not found", async () => {
    const api = setup();
    const reply = await api.call("GET", "/nowhere");
    expect(reply.status).toBe(404);
    expect(code(reply)).toBe("not_found");
  });

  test("a method a path does not take is not allowed, and Allow names those it takes", async () => {
    const api = setup();
    const reply = await api.call("PUT", "/users");
    expect(reply.status).toBe(405);
    expect(code(reply)).toBe("method_not_allowed");
    expect(reply.headers.get("Allow")).toBe("POST");
    const projects = await api.call("DELETE", "/projects");
    expect(projects.headers.get("Allow")).toBe("GET, POST");
  });

  test("the method is checked before the token", async () => {
    const api = setup();
    const reply = await api.call("PUT", "/me");
    expect(reply.status).toBe(405);
  });

  test("a body that is not JSON, or not an object, is refused", async () => {
    const api = setup();
    const broken = await api.call("POST", "/users", { raw: "{email:" });
    const list = await api.call("POST", "/users", { body: ["ada@example.com"] });
    expect([broken.status, list.status]).toEqual([400, 400]);
    expect([code(broken), code(list)]).toEqual(["validation", "validation"]);
    expect(broken.body.error.field).toBeUndefined();
  });

  test("the token is checked before the project", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const web = await project(api, ada);
    const reply = await api.call("GET", `/projects/${web.id}`);
    expect(reply.status).toBe(401);
  });
});
