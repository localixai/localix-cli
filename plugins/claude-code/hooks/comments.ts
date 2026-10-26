// How the window marks what the person sent, after a call's own report.
const MARK = '### What the person sent from the window'
// What the window says the person did there (a tab closed, a page moved) is for the agent's next
// call; only a batch of comments is worth a turn of its own.
const COMMENTS = 'The person sent '

/** The comments `llx window` printed, or undefined when it printed none. */
export function sentFrom(text: string): string | undefined {
  const at = text.indexOf(MARK)
  if (at < 0) return undefined
  const sent = text.slice(at)
  return sent.includes(COMMENTS) ? sent : undefined
}

/** The prompt a batch of comments becomes. */
export function promptFor(sent: string): string {
  return `The person sent comments from the llx window. Act on them (the llx:guide skill says how to call \`llx window\`).\n\n${sent}`
}
