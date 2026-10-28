//! The engine's side of a script: a fresh QuickJS runtime, the prelude, the script as an async
//! function's body. A tool call returns a promise at once; the call goes to the caller's thread,
//! and its answer settles the promise when it comes back, so the calls a script makes without
//! awaiting each one (`Promise.all`) are carried out together.

use std::sync::atomic::{AtomicU8, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex};

use rquickjs::context::EvalOptions;
use rquickjs::{Context, Ctx, Function, Promise, Runtime, Value};

use super::engine::{Call, Ended, Event, POLL, RUNNING};
use crate::shell::output::Output;

/// What one script may take: the engine fails an allocation past it.
const MEMORY: usize = 64 << 20;
/// The name the engine gives the script's source in an error's stack.
const SOURCE_FILE: &str = "exec-source";
/// What a script that did not parse is told to check.
const PARSE_HINT: &str = ". The script did not run: check that every `(`, `{`, `[` and string is \
closed where it should be; write long text as a template literal, its own backticks and `${` \
written \\` and \\${.";
/// What a script is told when a template literal ran the text it carries as code.
const TEMPLATE_HINT: &str = ". A `${...}` inside a template literal runs as code: write \\${ for \
the text `${`, and \\` for a backtick.";
/// How many characters of the line an error shows.
const SHOWN: usize = 160;

/// The script's lines of JavaScript before its own: `tools`, each call a promise kept until its
/// answer settles it, and `text`, over the host's functions, which it then hides but for
/// `__settle`, which the host takes away itself.
const PRELUDE: &str = r"
(() => {
  const call = globalThis.__call, said = globalThis.__text;
  delete globalThis.__call;
  delete globalThis.__text;
  const waiting = new Map();
  globalThis.__settle = (id, ok, value) => {
    const promise = waiting.get(id);
    waiting.delete(id);
    if (promise) ok ? promise.resolve(value) : promise.reject(new Error(value));
  };
  const tools = {};
  for (const name of globalThis.__names) {
    tools[name] = (args) => new Promise((resolve, reject) => {
      waiting.set(call(name, JSON.stringify(args === undefined ? {} : args)), { resolve, reject });
    });
  }
  delete globalThis.__names;
  globalThis.tools = Object.freeze(tools);
  globalThis.text = (value) => said(typeof value === 'string' ? value : String(JSON.stringify(value)));
})();
";

/// An answer to the call of that id: the tool's text, or the message its Error carries.
pub(super) type Answer = (u64, Result<String, String>);

/// Run `source`, sending each call on `events` and settling it from `answers`, until its promise
/// ends or `halt` is raised.
pub(super) fn evaluate(
    source: &str,
    tools: &[String],
    halt: &Arc<AtomicU8>,
    events: SyncSender<Event>,
    answers: &Receiver<Answer>,
) -> Result<String, Ended> {
    // The engine takes the source as a C string, which a NUL would end.
    if let Some((number, line)) = (source.lines().enumerate()).find(|(_, line)| line.contains('\0'))
    {
        return Err(Ended::Threw(format!(
            "the script holds a NUL character, which cannot run (line {}: {}): write that part \
             another way",
            number + 1,
            cut(line.trim()).replace('\0', "\\0")
        )));
    }
    let failed = |error: rquickjs::Error| Ended::Threw(error.to_string());
    let runtime = Runtime::new().map_err(failed)?;
    runtime.set_memory_limit(MEMORY);
    let raised = Arc::clone(halt);
    runtime.set_interrupt_handler(Some(Box::new(move || {
        raised.load(Ordering::SeqCst) != RUNNING
    })));
    let context = Context::full(&runtime).map_err(failed)?;
    let said = Arc::new(Mutex::new(Output::default()));
    context.with(|ctx| {
        let thrown = |error: rquickjs::Error| thrown(&ctx, &error, source);
        let settle = install(&ctx, tools, events, &said).map_err(thrown)?;
        let body = format!("(async () => {{\n{source}\n}})()");
        let mut options = EvalOptions::default();
        options.filename = Some(SOURCE_FILE.into());
        let promise: Promise = ctx.eval_with_options(body, options).map_err(thrown)?;
        let result = loop {
            while ctx.execute_pending_job() {}
            if let Some(result) = promise.result::<Value>() {
                break result.map_err(thrown)?;
            }
            match answers.recv_timeout(POLL) {
                Ok((id, answer)) => {
                    let (ok, text) = answer.map_or_else(|why| (false, why), |text| (true, text));
                    settle.call::<_, ()>((id, ok, text)).map_err(thrown)?;
                }
                // Raised while the script waits on a call: the caller says how it ended.
                Err(RecvTimeoutError::Timeout) if halt.load(Ordering::SeqCst) != RUNNING => {
                    return Err(Ended::Stopped);
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return Err(Ended::Stopped),
            }
        };
        let last = shown(&ctx, &result).map_err(thrown)?;
        let mut said = crate::lock(&said);
        if let Some(last) = last {
            said.push(last.as_bytes());
        }
        Ok(said.text())
    })
}

/// `__call`, `__text` and `__names` for the prelude, then the prelude; its `__settle`, taken off
/// the script's globals.
fn install<'js>(
    ctx: &Ctx<'js>,
    tools: &[String],
    events: SyncSender<Event>,
    said: &Arc<Mutex<Output>>,
) -> rquickjs::Result<Function<'js>> {
    let globals = ctx.globals();
    let made = AtomicU64::new(0);
    let call = move |name: String, arguments: String| -> u64 {
        let id = made.fetch_add(1, Ordering::SeqCst);
        // The caller is gone only when the run ended: the promise is left to the interrupt.
        let _ = events.send(Event::Call(Call {
            id,
            name,
            arguments,
        }));
        id
    };
    globals.set("__call", Function::new(ctx.clone(), call)?)?;
    let said = Arc::clone(said);
    let text = move |line: String| {
        let mut said = crate::lock(&said);
        said.push(line.as_bytes());
        said.push(b"\n");
    };
    globals.set("__text", Function::new(ctx.clone(), text)?)?;
    globals.set("__names", tools.to_vec())?;
    ctx.eval::<(), _>(PRELUDE)?;
    let settle = globals.get("__settle")?;
    globals.remove("__settle")?;
    Ok(settle)
}

/// What a script returned, as the model reads it: a string as it is, nothing for `undefined`,
/// anything else as JSON.
fn shown<'js>(ctx: &Ctx<'js>, value: &Value<'js>) -> rquickjs::Result<Option<String>> {
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }
    if let Some(text) = value.as_string() {
        return text.to_string().map(Some);
    }
    ctx.json_stringify(value.clone())?
        .map(|json| json.to_string())
        .transpose()
}

/// The script's error as one line: its name and message, as JavaScript says them, and where in
/// the script it is when the engine knows.
fn thrown(ctx: &Ctx<'_>, error: &rquickjs::Error, source: &str) -> Ended {
    let rquickjs::Error::Exception = error else {
        return Ended::Threw(error.to_string());
    };
    let caught = ctx.catch();
    let message = match caught.as_exception() {
        Some(exception) => {
            let name: Option<String> = exception.get("name").ok();
            let message = exception.message().unwrap_or_default();
            let line = (exception.stack().as_deref()).and_then(|stack| place(stack, source));
            let name = name.unwrap_or_else(|| "Error".into());
            // What to look at, since the engine names only a token or a name.
            let hint = match (name.as_str(), &line) {
                ("SyntaxError", _) => PARSE_HINT,
                ("ReferenceError", Some((_, text))) if text.contains("${") => TEMPLATE_HINT,
                _ => "",
            };
            let place = line.map_or_else(String::new, |(number, text)| {
                format!(" (line {number}: {})", cut(text.trim()))
            });
            format!("{name}: {message}{place}{hint}")
        }
        None => match caught.as_string().and_then(|text| text.to_string().ok()) {
            Some(text) => text,
            None => "the script threw".into(),
        },
    };
    Ended::Threw(message)
}

/// Where in the script the engine's stack puts an error: the line's number and text, as the
/// model wrote it (the async function around the script adds one line before it).
fn place<'a>(stack: &str, source: &'a str) -> Option<(usize, &'a str)> {
    let at = stack
        .lines()
        .find_map(|frame| frame.split_once(SOURCE_FILE))?
        .1;
    let line: usize = at.trim_start_matches(':').split(':').next()?.parse().ok()?;
    let line = line.checked_sub(1)?;
    Some((line, source.lines().nth(line.checked_sub(1)?)?))
}

/// A line as an error shows it: its start, when it is long (a whole file on one line).
fn cut(text: &str) -> String {
    let start: String = text.chars().take(SHOWN).collect();
    match start.len() < text.len() {
        true => format!("{start}…"),
        false => start,
    }
}
