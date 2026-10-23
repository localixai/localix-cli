//! A terminal session through `bash` (`BashTool::terminal`, the window's terminal tabs): a
//! program that must ask runs in a session that stays open between calls. The arguments are checked before the person is asked and before anything is typed;
//! starting a session and typing into it ask the gate, and the question shows exactly what is
//! typed; reading a session and closing it ask nothing, since they only end or read what the
//! person already approved.

use contracts::tool::{ToolError, ToolOutput};

use super::bash::{Args, BashTool};

#[cfg(not(unix))]
pub(super) fn run(_: &BashTool, _: &Args) -> Result<(ToolOutput, Option<u32>), ToolError> {
    Err(ToolError::Failed(
        "terminal sessions need a unix system; pass the program's non-interactive flags instead"
            .into(),
    ))
}

#[cfg(unix)]
pub(super) use unix::run;

#[cfg(unix)]
mod unix {
    use super::{Args, BashTool, ToolError, ToolOutput};
    use crate::ask;
    use crate::shell::bash::{EMPTY, NAME, summary};
    use crate::shell::keys::{self, NAMES};
    use crate::shell::process::Env;
    use crate::shell::report::{report, typed};
    use crate::shell::session::{Until, Wait};

    /// The longest input typed at once: a longer text is better read from a file.
    const MAX_INPUT: usize = 4096;
    /// The longest line of input. A terminal that hands a program whole lines (canonical mode)
    /// keeps 1023 bytes of one on macOS and silently drops the rest, Enter included, so the program
    /// would wait for a line end that never comes while every byte counts as typed. Below that,
    /// with room for keys pressed after the text.
    const MAX_LINE: usize = 1000;

    pub(crate) fn run(
        tool: &BashTool,
        args: &Args,
    ) -> Result<(ToolOutput, Option<u32>), ToolError> {
        check(args)?;
        let input = args.input.as_deref().unwrap_or_default();
        let keys = args.keys.as_deref().unwrap_or_default();
        let wait = Wait {
            needle: args.wait_for.as_deref().filter(|needle| !needle.is_empty()),
            yield_ms: args.yield_ms,
        };
        let failed = ToolError::Failed;
        let mut started = None;
        let report = match args.session {
            None => {
                let command = args.command.as_deref().unwrap_or_default();
                let (dir, place) = tool.dir(args.workdir.as_deref())?;
                tool.sessions.room().map_err(failed)?;
                let place = match &place {
                    Some(place) => format!("in a terminal, in {place}"),
                    None => "in a terminal".to_string(),
                };
                ask(tool.gate, NAME, &format!("{}  ({place})", summary(command)))?;
                let names = (tool.hidden)();
                let hidden: Vec<&str> = names.iter().map(String::as_str).collect();
                let env = Env {
                    vars: tool.workspace.env(),
                    hidden: &hidden,
                };
                let turn = tool
                    .sessions
                    .start(command, &dir, env, wait, args.show, tool.stop)
                    .map_err(failed)?;
                if !matches!(turn.until, Until::Exited(_) | Until::Closed { .. }) {
                    started = Some(turn.id);
                }
                report(&turn, Some(&place), wait.needle, "")
            }
            Some(id) if args.close => {
                report(&tool.sessions.close(id).map_err(failed)?, None, None, "")
            }
            Some(id) => {
                let command = tool.sessions.command(id).map_err(failed)?;
                if !input.is_empty() || !keys.is_empty() {
                    let typed = typed(input, keys);
                    ask(
                        tool.gate,
                        NAME,
                        &format!("type into session {id} (`{command}`): {typed}"),
                    )?;
                }
                let turn = tool
                    .sessions
                    .send(id, &keys::typed(input), keys, wait, args.show, tool.stop)
                    .map_err(failed)?;
                report(&turn, None, wait.needle, input)
            }
        };
        Ok((report.into(), started))
    }

    /// Every problem with the arguments, before anything is asked or typed.
    fn check(args: &Args) -> Result<(), ToolError> {
        let refuse = |why: &str| Err(ToolError::Failed(why.into()));
        let blank = args
            .command
            .as_deref()
            .is_none_or(|command| command.trim().is_empty());
        let typing = args.input.is_some() || args.keys.is_some();
        if args.command.is_some() && args.session.is_some() {
            return refuse("pass command to start, or session to continue, not both");
        }
        if args.session.is_none() && blank {
            return refuse(EMPTY);
        }
        if args.timeout_ms.is_some() && (args.tty || args.session.is_some()) {
            return refuse(
                "timeout_ms is for commands run to their end; a session waits yield_ms per call \
                 and ends with close",
            );
        }
        if !args.tty && args.session.is_none() {
            return refuse(
                "input, keys, wait_for, yield_ms, close and show are for terminal sessions: set \
                 tty to start one, or pass session",
            );
        }
        if typing && args.tty {
            return refuse("start the session first, then send input with session");
        }
        if args.session.is_some() && args.workdir.is_some() {
            return refuse(
                "a session keeps the directory it started in: workdir is for starting one",
            );
        }
        if args.close && (typing || args.wait_for.is_some()) {
            return refuse("close ends the session: send input in an earlier call");
        }
        if args.close && args.session.is_none() {
            return refuse("close ends a running session: pass its id as session");
        }
        let keys = args.keys.as_deref().unwrap_or_default();
        if let Some(unknown) = keys.iter().find(|key| keys::encode(key, false).is_none()) {
            return refuse(&format!("unknown key \"{unknown}\": use {NAMES}"));
        }
        if args
            .input
            .as_deref()
            .is_some_and(|input| input.len() > MAX_INPUT)
        {
            return refuse(&format!(
                "input is over {MAX_INPUT} bytes: write it to a file and have the program read \
                 the file"
            ));
        }
        if args
            .input
            .as_deref()
            .is_some_and(|input| input.split(['\n', '\r']).any(|line| line.len() > MAX_LINE))
        {
            return refuse(&format!(
                "a line of input is over {MAX_LINE} bytes, and a terminal drops the rest of a \
                 longer line, Enter included: break it into lines, or write it to a file and have \
                 the program read the file"
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "interactive_tests.rs"]
mod tests;
