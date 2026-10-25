//! `llx mcp`: the window served to another agent over MCP on stdio (ARCHITECTURE.md W9). The
//! daemon is found, else started, as for a session at the terminal; this process is its tool
//! client, the interface in `mcp`, until the agent closes its stdin. Nothing else is written
//! to stdout: it is the agent's.

use crate::error::Failure;
use crate::{daemon, exit};

pub(crate) fn mcp() -> Result<i32, Failure> {
    let config = daemon::config().map_err(Failure::Environment)?;
    let path = daemon::reach(&config).map_err(Failure::Environment)?;
    // What the daemon says beside its answers (none to a tool client) is nothing to it.
    let client = client::Client::connect(&path, |_| {})
        .map_err(|error| Failure::Environment(format!("the daemon did not attach ({error})")))?;
    let (input, output) = (std::io::stdin().lock(), std::io::stdout());
    crate::mcp::run(&client, crate::mcp::here(), daemon::build(), input, output)
        .map_err(|error| Failure::Environment(format!("llx mcp: {error}")))?;
    Ok(exit::OK)
}
