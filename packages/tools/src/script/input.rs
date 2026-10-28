//! What a call to `exec` carries: the script's text as the model wrote it, nothing to escape, and
//! the grammar a provider that holds a model to one is given: an optional first line
//! `// @exec: {...}`, then the source (ARCHITECTURE.md X4).

use contracts::tool::{Grammar, Input, ToolError};
use serde_json::Value;

/// The grammar in Lark syntax: a pragma line, then any source.
const LARK: &str = "\nstart: pragma_source | plain_source\npragma_source: PRAGMA_LINE NEWLINE SOURCE\n\
plain_source: SOURCE\n\nPRAGMA_LINE: /[ \\t]*\\/\\/ @exec:[^\\r\\n]*/\nNEWLINE: /\\r?\\n/\n\
SOURCE: /[\\s\\S]+/\n";

pub(super) fn input() -> Input {
    Input::Text(Grammar::Lark(LARK.into()))
}

/// The script a call carries.
pub(super) fn source(arguments: &Value) -> Result<&str, ToolError> {
    arguments.as_str().ok_or_else(|| {
        ToolError::Failed(
            "exec takes the script as its text, written as it is (JavaScript, not JSON): call \
             it again with the script alone"
                .into(),
        )
    })
}
