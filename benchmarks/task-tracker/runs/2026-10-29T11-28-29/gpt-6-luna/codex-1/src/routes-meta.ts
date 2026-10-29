export function methods(path: string) {
  if (path === "/users" || path === "/sessions")
    return path === "/users" ? ["POST"] : ["POST", "DELETE"];
  if (path === "/me") return ["GET"];
  if (path === "/projects") return ["GET", "POST"];
  if (/^\/projects\/\d+$/.test(path)) return ["GET"];
  if (/^\/projects\/\d+\/members$/.test(path)) return ["GET", "POST"];
  if (/^\/projects\/\d+\/members\/\d+$/.test(path)) return ["DELETE"];
  if (/^\/projects\/\d+\/tasks$/.test(path)) return ["GET", "POST"];
  if (/^\/projects\/\d+\/tasks\/bulk$/.test(path)) return ["POST"];
  if (/^\/projects\/\d+\/stats$/.test(path)) return ["GET"];
  if (/^\/tasks\/[^/]+$/.test(path)) return ["GET", "PATCH", "DELETE"];
  if (
    /^\/tasks\/[^/]+\/(transitions|dependencies|history|comments)(\/[^/]+)?$/.test(
      path,
    )
  ) {
    if (path.includes("transitions")) return ["POST"];
    if (path.includes("dependencies"))
      return path.endsWith("/dependencies") ? ["GET", "POST"] : ["DELETE"];
    if (path.includes("history")) return ["GET"];
    return path.endsWith("/comments") ? ["GET", "POST"] : ["GET"];
  }
  return undefined;
}
