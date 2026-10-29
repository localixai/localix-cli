import { fail, validation } from "./http";
import type { Context, User } from "./model";
export function signup(c: Context, body: Record<string, any>) {
  const email = typeof body.email === "string" ? body.email.trim().toLowerCase() : "";
  const password = body.password;
  const name = typeof body.name === "string" ? body.name.trim() : "";
  if (!/^.+@.+\..+$/.test(email)) validation("email");
  if (typeof password !== "string" || password.length < 8) validation("password");
  if (!name || name.length > 100) validation("name");
  if (c.s.users.some((u) => u.email === email)) fail(409, "conflict", "Email already exists", "email");
  const user: User = { id: ++c.s.ids.user, email, name, password };
  c.s.users.push(user);
  return { id: user.id, email, name };
}
export function session(c: Context, body: Record<string, any>) {
  const email = typeof body.email === "string" ? body.email.toLowerCase() : "";
  const user = c.s.users.find((u) => u.email === email && u.password === body.password);
  if (!user) fail(401, "unauthorized", "Invalid email or password");
  const token = `session-${user!.id}-${crypto.randomUUID()}`;
  c.s.tokens.set(token, user!.id);
  return { token };
}
export function authenticate(c: Context, request: Request): User {
  const header = request.headers.get("Authorization") || "";
  const match = /^Bearer (.+)$/.exec(header);
  const token = match?.[1] ?? "";
  const id = match ? c.s.tokens.get(token) : undefined;
  const user = c.s.users.find((u) => u.id === id);
  if (!user) fail(401, "unauthorized", "Authentication required");
  const minute = Math.floor(c.now() / 60000);
  const rate = c.s.limits.get(token);
  if (!rate || rate.minute !== minute) c.s.limits.set(token, { minute, count: 1 });
  else if (rate.count >= 100) {
    const retry = Math.ceil(((minute + 1) * 60000 - c.now()) / 1000);
    fail(429, "rate_limited", "Rate limit exceeded");
  } else rate.count++;
  return user!;
}
export function rateHeaders(c: Context, token: string | null) {
  if (!token || !c.s.tokens.has(token)) return {};
  const minute = Math.floor(c.now() / 60000);
  const rate = c.s.limits.get(token);
  return { "X-RateLimit-Limit": "100", "X-RateLimit-Remaining": String(
    Math.max(0, 100 - (rate?.minute === minute ? rate.count : 0))) };
}
export function tokenFrom(request: Request) {
  const m = /^Bearer (.+)$/.exec(request.headers.get("Authorization") || "");
  return m?.[1] ?? null;
}
