import { describe, expect, test } from "bun:test";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";

const SRC = join(import.meta.dir, "..", "src");
const MAX_LINES = 200;
const MAX_WIDTH = 120;

function sources(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return sources(path);
    return name.endsWith(".ts") && !name.endsWith(".test.ts") ? [path] : [];
  });
}

describe("structure", () => {
  test(`every source file in src/ holds at most ${MAX_LINES} lines`, () => {
    const long = sources(SRC)
      .map((path) => [path.slice(SRC.length + 1), readFileSync(path, "utf8").split("\n").length] as const)
      .filter(([, lines]) => lines > MAX_LINES);
    expect(long).toEqual([]);
  });

  test(`no line in src/ is wider than ${MAX_WIDTH} characters`, () => {
    const wide = sources(SRC).flatMap((path) =>
      readFileSync(path, "utf8")
        .split("\n")
        .flatMap((line, n) => (line.length > MAX_WIDTH ? [`${path.slice(SRC.length + 1)}:${n + 1}`] : [])),
    );
    expect(wide).toEqual([]);
  });

  test("src/app.ts is where the app starts", () => {
    expect(sources(SRC).map((path) => path.slice(SRC.length + 1))).toContain("app.ts");
  });
});
