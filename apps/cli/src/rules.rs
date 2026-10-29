//! `--allow` and `--deny`: rules on what runs without asking, read as Claude Code writes them
//! (`bash(cargo test *)`, `apply_patch(docs/**)`, `jobs`). A deny rule refuses what it names in
//! every mode; an allow rule lets what it names run where the mode would ask or, with no one to
//! ask, refuse. A read-only mode stays read-only whatever is allowed. They come from the command
//! line only: a rule read from the repository would let a pull request allow itself.

mod command;
mod git;
mod path;

/// The tools a rule can name, and whether a pattern can narrow it: a command's text for `bash`,
/// a path for `apply_patch`. The others are named whole.
const TOOLS: [(&str, bool); 3] = [("bash", true), ("apply_patch", true), ("jobs", false)];

/// What the rules say of an effect before the mode does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Ruling {
    Allowed,
    /// Refused, and why, naming the rule.
    Denied(String),
    /// No rule settles it: the mode decides.
    Unsettled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Rule {
    text: String,
    tool: &'static str,
    /// `None`: every call of the tool.
    pattern: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Rules {
    allow: Vec<Rule>,
    deny: Vec<Rule>,
}

impl Rules {
    /// The rules as the flags give them; an error names the rule it could not read.
    pub(crate) fn new(allow: &[String], deny: &[String]) -> Result<Self, String> {
        let read = |list: &[String]| {
            list.iter()
                .map(|text| rule(text))
                .collect::<Result<Vec<_>, _>>()
        };
        Ok(Self {
            allow: read(allow)?,
            deny: read(deny)?,
        })
    }

    /// These rules and `more`'s, as one set: a mode's and the command line's.
    pub(crate) fn with(&self, more: &Self) -> Self {
        let join = |a: &[Rule], b: &[Rule]| [a, b].concat();
        Self {
            allow: join(&self.allow, &more.allow),
            deny: join(&self.deny, &more.deny),
        }
    }

    /// What the rules say of `tool` acting on `subject` (its command, or its path; empty when
    /// the call has none a pattern could judge). Deny first, then allow.
    pub(crate) fn judge(&self, tool: &str, subject: &str) -> Ruling {
        let ours = |rules: &[Rule]| {
            rules
                .iter()
                .filter(|rule| rule.tool == tool)
                .cloned()
                .collect::<Vec<_>>()
        };
        let (allow, deny) = (ours(&self.allow), ours(&self.deny));
        if let Some(rule) = deny.iter().find(|rule| denies(rule, subject)) {
            return Ruling::Denied(format!("refused by the rule `{}`", rule.text));
        }
        match allows(&allow, tool, subject) {
            true => Ruling::Allowed,
            false => Ruling::Unsettled,
        }
    }

    /// The rules as the model reads them in its context, or `None` with none given.
    pub(crate) fn describe(&self) -> Option<String> {
        if self.allow.is_empty() && self.deny.is_empty() {
            return None;
        }
        let list = |rules: &[Rule]| {
            rules
                .iter()
                .map(|rule| format!("`{}`", rule.text))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut said = vec![];
        if !self.allow.is_empty() {
            said.push(format!(
                "Allowed without asking: {}. A command is allowed only when every part of it \
                 (split at `&&`, `||`, `;`, `|`) matches one, with no `$(…)`, backticks or \
                 redirect to a file.",
                list(&self.allow)
            ));
        }
        if !self.deny.is_empty() {
            said.push(format!("Always refused: {}.", list(&self.deny)));
        }
        let text = said.join(" ");
        Some(text)
    }
}

fn denies(rule: &Rule, subject: &str) -> bool {
    match (&rule.pattern, rule.tool) {
        (None, _) => true,
        (Some(_), _) if subject.is_empty() => false,
        (Some(pattern), "apply_patch") => path::matches(pattern, subject),
        (Some(pattern), _) => command::every(subject)
            .iter()
            .any(|part| command::matches(pattern, part)),
    }
}

fn allows(rules: &[Rule], tool: &str, subject: &str) -> bool {
    if rules.iter().any(|rule| rule.pattern.is_none()) {
        return true;
    }
    let patterns: Vec<&str> = rules
        .iter()
        .filter_map(|rule| rule.pattern.as_deref())
        .collect();
    if patterns.is_empty() || subject.is_empty() {
        return false;
    }
    if tool == "apply_patch" {
        return patterns
            .iter()
            .any(|pattern| path::matches(pattern, subject));
    }
    let Some(parts) = command::sure(subject) else {
        return false;
    };
    parts.iter().all(|part| {
        patterns
            .iter()
            .any(|pattern| command::matches(pattern, part))
    })
}

/// `tool` or `tool(pattern)`; the tool's name in any case, `tool(*)` the same as `tool`.
fn rule(text: &str) -> Result<Rule, String> {
    let wrong = |why: &str| format!("the rule `{text}` cannot be read: {why}");
    let trimmed = text.trim();
    let (name, pattern) = match trimmed.split_once('(') {
        Some((name, rest)) => {
            let pattern = rest
                .strip_suffix(')')
                .ok_or_else(|| wrong("it has no closing `)`"))?;
            (name, Some(pattern.trim()))
        }
        None => (trimmed, None),
    };
    let name = name.trim().to_ascii_lowercase();
    let known = TOOLS
        .iter()
        .map(|(tool, _)| *tool)
        .collect::<Vec<_>>()
        .join(", ");
    let Some(&(tool, narrowed)) = TOOLS.iter().find(|(tool, _)| *tool == name) else {
        return Err(wrong(&format!("a rule names one of {known}")));
    };
    let pattern = match pattern {
        None | Some("*") => None,
        Some("") => return Err(wrong("the parentheses are empty")),
        Some(_) if !narrowed => {
            return Err(wrong(&format!(
                "`{tool}` is named whole, without a pattern"
            )));
        }
        Some(pattern) => {
            if let Some(why) = (tool == "apply_patch")
                .then(|| path::invalid(pattern))
                .flatten()
            {
                return Err(wrong(why));
            }
            Some(pattern.to_string())
        }
    };
    Ok(Rule {
        text: trimmed.to_string(),
        tool,
        pattern,
    })
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
