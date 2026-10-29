export type DB = ReturnType<typeof state>;
export function state(now: () => number) {
  return {
    now, users: [] as any[], projects: [] as any[], tasks: [] as any[],
    tokens: new Map<string, number>(), counters: { user: 0, project: 0, task: 0, comment: 0 },
    limits: new Map<string, { minute: number; count: number }>(),
  };
}
