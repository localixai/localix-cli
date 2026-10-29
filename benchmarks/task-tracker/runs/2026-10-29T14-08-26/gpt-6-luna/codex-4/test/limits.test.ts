import { describe, expect, test } from "bun:test";
import { code, setup, signUp } from "./helpers";

describe("rate limits", () => {
  test("a token's requests say the limit and what is left of it this minute", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const first = await api.call("GET", "/me", { token: ada.token });
    const second = await api.call("GET", "/me", { token: ada.token });
    expect(first.headers.get("X-RateLimit-Limit")).toBe("100");
    expect(first.headers.get("X-RateLimit-Remaining")).toBe("99");
    expect(second.headers.get("X-RateLimit-Remaining")).toBe("98");
  });

  test("the 101st request of a minute waits for the next, as Retry-After says", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    api.advance(20_000);
    for (let n = 0; n < 100; n++) {
      const reply = await api.call("GET", "/me", { token: ada.token });
      if (reply.status !== 200) throw new Error(`request ${n + 1} answered ${reply.status}`);
    }
    const over = await api.call("GET", "/me", { token: ada.token });
    expect(over.status).toBe(429);
    expect(code(over)).toBe("rate_limited");
    expect(over.headers.get("Retry-After")).toBe("40");
    expect(over.headers.get("X-RateLimit-Remaining")).toBe("0");
    api.advance(40_000);
    const next = await api.call("GET", "/me", { token: ada.token });
    expect(next.status).toBe(200);
    expect(next.headers.get("X-RateLimit-Remaining")).toBe("99");
  });

  test("Retry-After rounds a part of a second up", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    api.advance(59_500);
    for (let n = 0; n < 100; n++) await api.call("GET", "/me", { token: ada.token });
    const over = await api.call("GET", "/me", { token: ada.token });
    expect(over.headers.get("Retry-After")).toBe("1");
  });

  test("each token has a limit of its own", async () => {
    const api = setup();
    const ada = await signUp(api, "Ada");
    const bob = await signUp(api, "Bob");
    for (let n = 0; n < 100; n++) await api.call("GET", "/me", { token: ada.token });
    expect((await api.call("GET", "/me", { token: ada.token })).status).toBe(429);
    expect((await api.call("GET", "/me", { token: bob.token })).status).toBe(200);
  });

  test("requests with no token are not counted, nor limited", async () => {
    const api = setup();
    for (let n = 0; n < 120; n++) await api.call("GET", "/me");
    const signed = await api.call("POST", "/users", {
      body: { email: "ada@example.com", password: "correct horse", name: "Ada" },
    });
    expect(signed.status).toBe(201);
  });
});
