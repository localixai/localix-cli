import { describe, expect, test } from "bun:test";
import { code, setup, signUp } from "./helpers";

describe("users", () => {
  test("signing up answers the user without the password", async () => {
    const api = setup();
    const reply = await api.call("POST", "/users", {
      body: { email: "ada@example.com", password: "correct horse", name: "Ada" },
    });
    expect(reply.status).toBe(201);
    expect(reply.body).toEqual({ id: 1, email: "ada@example.com", name: "Ada" });
  });

  test("users are numbered in the order they sign up", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const bob = await signUp(api, "Bob");
    expect([ada.id, bob.id]).toEqual([1, 2]);
  });

  test("an email is kept in lower case, and the name trimmed", async () => {
    const api = setup();
    const reply = await api.call("POST", "/users", {
      body: { email: "Ada@Example.COM", password: "correct horse", name: "  Ada  " },
    });
    expect(reply.body).toEqual({ id: 1, email: "ada@example.com", name: "Ada" });
  });

  test("an email already taken, in any case, is a conflict", async () => {
    const api = setup();
    await signUp(api, "Ada");
    const reply = await api.call("POST", "/users", {
      body: { email: "ADA@example.com", password: "another one", name: "Ada Again" },
    });
    expect(reply.status).toBe(409);
    expect(reply.body.error).toMatchObject({ code: "conflict", field: "email" });
  });

  test.each([
    ["an email with no @", { email: "ada.example.com", password: "correct horse", name: "Ada" }, "email"],
    ["an email with no dot after the @", { email: "ada@example", password: "correct horse", name: "Ada" }, "email"],
    ["a password under 8 characters", { email: "ada@example.com", password: "short", name: "Ada" }, "password"],
    ["an empty name", { email: "ada@example.com", password: "correct horse", name: "   " }, "name"],
    ["a name over 100 characters", { email: "ada@example.com", password: "correct horse", name: "a".repeat(101) }, "name"],
    ["a missing name", { email: "ada@example.com", password: "correct horse" }, "name"],
    ["an email that is not a string", { email: 42, password: "correct horse", name: "Ada" }, "email"],
  ])("%s is refused", async (_, body, field) => {
    const api = setup();
    const reply = await api.call("POST", "/users", { body });
    expect(reply.status).toBe(400);
    expect(reply.body.error).toMatchObject({ code: "validation", field });
    expect(typeof reply.body.error.message).toBe("string");
  });
});

describe("sessions", () => {
  test("signing in answers a token that /me reads the user from", async () => {
    const api = setup();
    await signUp(api, "Ada");
    const session = await api.call("POST", "/sessions", {
      body: { email: "ADA@example.com", password: "correct horse" },
    });
    expect(session.status).toBe(201);
    expect(typeof session.body.token).toBe("string");
    const me = await api.call("GET", "/me", { token: session.body.token });
    expect(me.status).toBe(200);
    expect(me.body).toEqual({ id: 1, email: "ada@example.com", name: "Ada" });
  });

  test("a wrong password or an unknown email is unauthorized", async () => {
    const api = setup();
    await signUp(api, "Ada");
    const wrong = await api.call("POST", "/sessions", { body: { email: "ada@example.com", password: "wrong horse" } });
    const unknown = await api.call("POST", "/sessions", { body: { email: "eve@example.com", password: "correct horse" } });
    expect([wrong.status, unknown.status]).toEqual([401, 401]);
    expect([code(wrong), code(unknown)]).toEqual(["unauthorized", "unauthorized"]);
  });

  test("each sign-in is a token of its own", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const again = await api.call("POST", "/sessions", { body: { email: ada.email, password: "correct horse" } });
    expect(again.body.token).not.toBe(ada.token);
  });

  test("no token, a token of another scheme, or an unknown token is unauthorized", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const none = await api.call("GET", "/me");
    const basic = await api.call("GET", "/me", { headers: { Authorization: `Basic ${ada.token}` } });
    const unknown = await api.call("GET", "/me", { token: "not-a-token" });
    expect([none.status, basic.status, unknown.status]).toEqual([401, 401, 401]);
    expect(code(none)).toBe("unauthorized");
  });

  test("signing out ends that token only", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const other = await api.call("POST", "/sessions", { body: { email: ada.email, password: "correct horse" } });
    const out = await api.call("DELETE", "/sessions", { token: ada.token });
    expect(out.status).toBe(204);
    expect(out.body).toBeNull();
    expect((await api.call("GET", "/me", { token: ada.token })).status).toBe(401);
    expect((await api.call("GET", "/me", { token: other.body.token })).status).toBe(200);
  });
});
