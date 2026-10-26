//! `llx window`: one call of the agent's `window` from another agent's shell (ARCHITECTURE.md W9).
//! The daemon is found, else started, as for a session at the terminal; this process is its tool
//! client, the interface in `call`, for one call. Only the answer is written to stdout: it is the
//! agent's.

use std::io::Read;
use std::path::Path;

use crate::call::{self, Asked, Came};
use crate::cli::WindowArgs;
use crate::error::Failure;
use crate::{daemon, exit};

pub(crate) fn window(args: &WindowArgs) -> Result<i32, Failure> {
    let asked = asked(args)?;
    let config = daemon::config().map_err(Failure::Environment)?;
    // The owner's end reaches the daemon there is, whatever its build: after an update its tabs
    // are what keeps the older one in use. With none running there is nothing to end.
    let (path, build) = match asked {
        Asked::Release => {
            let path = daemon::socket_path(&config);
            match daemon::status(&path, ASK) {
                Some(found) => (path, found.build),
                None => return Ok(exit::OK),
            }
        }
        Asked::Describe | Asked::Call(_) => {
            let path = daemon::reach(&config).map_err(|why| Failure::Environment(hinted(why)))?;
            (path, daemon::build())
        }
    };
    let client = client::Client::connect(&path, |_| {})
        .map_err(|error| Failure::Environment(format!("the daemon did not attach ({error})")))?;
    let here = std::env::current_dir().unwrap_or_default();
    let owner = (args.owner.clone()).unwrap_or_else(|| project(&here).display().to_string());
    // Beside the daemon's socket, in a directory only this user reads.
    let shots = path.parent().unwrap_or(&here).join("shots");
    let place = call::here(&owner);
    let mut out = std::io::stdout().lock();
    let came = call::run(&client, place, build, asked, &shots, &mut out)
        .map_err(|why| Failure::Environment(format!("llx window: {why}")))?;
    Ok(match came {
        Came::Done => exit::OK,
        Came::Failed => exit::FAILED,
    })
}

/// How long a running daemon has to say what it is.
const ASK: std::time::Duration = std::time::Duration::from_secs(5);

/// `why` the daemon could not be reached, with the way on when an older one is in use: its tabs
/// hold it, and ending them lets the next call start this build.
fn hinted(why: String) -> String {
    match why.ends_with("in use") {
        true => format!(
            "{why}: `llx window --end` closes this project's tabs there (`--as NAME --end` \
             another owner's), then the next call starts this build; `llx serve status` says \
             what else holds it"
        ),
        false => why,
    }
}

/// What the command line asks: the call's arguments (from stdin for `-`), the end of the owner's
/// tabs, or with neither how to call it.
fn asked(args: &WindowArgs) -> Result<Asked, Failure> {
    if args.end {
        return Ok(Asked::Release);
    }
    let Some(text) = &args.call else {
        return Ok(Asked::Describe);
    };
    let text = match text.as_str() {
        "-" => {
            let mut read = String::new();
            (std::io::stdin().take(1 << 20).read_to_string(&mut read))
                .map_err(|error| Failure::Usage(format!("stdin could not be read ({error})")))?;
            read
        }
        given => given.to_string(),
    };
    match serde_json::from_str(&text) {
        Ok(arguments @ serde_json::Value::Object(_)) => Ok(Asked::Call(arguments)),
        Ok(_) | Err(_) => Err(Failure::Usage(
            "the call is the `window` tool's arguments as one JSON object, such as \
             '{\"action\": \"list\"}'; `llx window` alone says what they are"
                .into(),
        )),
    }
}

/// The project `dir` is in: the nearest directory with a `.git` at or above it, else `dir`.
fn project(dir: &Path) -> &Path {
    dir.ancestors()
        .find(|at| at.join(".git").exists())
        .unwrap_or(dir)
}
