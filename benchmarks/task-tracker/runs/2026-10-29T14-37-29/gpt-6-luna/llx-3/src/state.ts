export type State = {
  now: () => number;
  users: any[];
  sessions: Map<string, number>;
  projects: any[];
  tasks: any[];
  comments: any[];
  history: any[];
  dependencies: Set<string>;
  counts: Map<number, number>;
  rates: Map<string, { minute: number; count: number }>;
  idem: Map<string, any>;
};
export const createState = (now: () => number): State => ({
  now, users: [], sessions: new Map(), projects: [], tasks: [], comments: [],
  history: [], dependencies: new Set(), counts: new Map(), rates: new Map(),
  idem: new Map(),
});
