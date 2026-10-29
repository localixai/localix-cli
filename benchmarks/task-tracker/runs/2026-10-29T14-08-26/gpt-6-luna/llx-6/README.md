# Task tracker

A JSON API for projects and their tasks, in TypeScript on Bun, with no dependencies.

`createApp` in `src/app.ts` returns the app; its `fetch` answers one HTTP request. The tests in
`test/` say what the API does: every route, status, body and header.

The code lives in modules under `src/`: no file over 200 lines, no line over 120 characters
(`test/structure.test.ts`).

```sh
bun install
bun test              # the tests
bunx tsc --noEmit     # the types
```
