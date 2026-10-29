import { fail } from "./http";
import type { Context, Project, User } from "./model";
import { clean, createTask } from "./taskops";
import { plainTask } from "./http";
export function createWithIdempotency(c: Context, p: Project, user: User,
  body: Record<string, any>, key: string | null) {
  const values = clean(body, c, p);
  if (!key) return plainTask(createTask(c, p, user, values));
  const storageKey = `${user.id}:${key}`;
  const prior = c.s.idem.get(storageKey);
  const signature = JSON.stringify(body);
  if (prior && c.now() - prior.at < 86400000) {
    if (prior.signature !== signature) fail(422, "idempotency_mismatch", "Key was used with another request");
    return prior.body;
  }
  const created = plainTask(createTask(c, p, user, values));
  c.s.idem.set(storageKey, { at: c.now(), signature, body: created });
  return created;
}
