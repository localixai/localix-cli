//! `apply_patch`: the files changed by a patch, `*** Begin Patch` … `*** End Patch`. It is offered
//! to the model as free text, held to the patch's grammar where the provider takes one, so a
//! patch comes with nothing escaped. A patch says the lines it changes with the lines around them,
//! and fails loudly when the model's picture of the file is stale, so the model reads again
//! instead of changing the wrong place.

use contracts::gate::Gate;
use contracts::tool::{Grammar, Input, Tool, ToolError, ToolOutput, ToolSpec};
use serde_json::Value;

use crate::workspace::Workspace;

const NAME: &str = "apply_patch";

const DESCRIPTION: &str = "Edit files with a patch. This is a free-text tool: write the patch \
itself, not JSON. It starts with `*** Begin Patch` and ends with `*** End Patch`; between them, \
for each file, `*** Add File: <path>` and its lines each starting with `+`; `*** Delete File: \
<path>`; or `*** Update File: <path>`, optionally `*** Move to: <path>`, then hunks: a line `@@` \
(or `@@ <a line near the change>`), then the hunk's lines, each starting with ` ` (kept, three \
around each change), `-` (removed) or `+` (added). Paths are relative to the workspace root. \
Files change only inside the workspace; missing directories are created and line endings are \
kept. Read a file before changing it, and again when a patch fails.";

/// The patch's grammar, as the model's provider holds it to.
const GRAMMAR: &str = r#"start: begin_patch hunk+ end_patch
begin_patch: "*** Begin Patch" LF
end_patch: "*** End Patch" LF?

hunk: add_hunk | delete_hunk | update_hunk
add_hunk: "*** Add File: " filename LF add_line+
delete_hunk: "*** Delete File: " filename LF
update_hunk: "*** Update File: " filename LF change_move? change?

filename: /(.+)/
add_line: "+" /(.*)/ LF -> line

change_move: "*** Move to: " filename LF
change: (change_context | change_line)+ eof_line?
change_context: ("@@" | "@@ " /(.+)/) LF
change_line: ("+" | "-" | " ") /(.*)/ LF
eof_line: "*** End of File" LF

%import common.LF
"#;

pub struct PatchTool<'a> {
    pub workspace: &'a Workspace,
    pub gate: &'a dyn Gate,
}

impl Tool for PatchTool<'_> {
    fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: NAME.into(),
            description: DESCRIPTION.into(),
            input: Input::Text(Grammar::Lark(GRAMMAR.into())),
        }
    }

    fn call(&self, arguments: &Value) -> Result<ToolOutput, ToolError> {
        let Some(patch) = super::text(arguments) else {
            return Err(ToolError::Failed(
                "apply_patch takes a patch as text, not JSON: `*** Begin Patch`, then a section \
                 for each file, then `*** End Patch`"
                    .into(),
            ));
        };
        super::run(self.workspace, self.gate, patch).map(Into::into)
    }
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
