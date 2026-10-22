//! `decision-loop skills`: the guides for agents, built into the binary so they always match it.
//! The `skills/decision-loop/SKILL.md` stub that agents install only points here.

const SKILLS: &[(&str, &str)] = &[
    ("core", include_str!("../skill-data/core/SKILL.md")),
    ("testing", include_str!("../skill-data/testing/SKILL.md")),
];

/// The `description:` line of a guide's frontmatter.
fn description(text: &str) -> &str {
    text.lines()
        .find_map(|line| line.strip_prefix("description: "))
        .unwrap_or("")
}

/// A guide without its frontmatter.
fn body(text: &str) -> &str {
    text.strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .map_or(text, |(_, body)| body.trim_start())
}

pub fn list() -> String {
    SKILLS
        .iter()
        .map(|(name, text)| format!("{name:<9} {}\n", description(text)))
        .collect()
}

pub fn get(name: &str) -> Option<&'static str> {
    SKILLS
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, text)| body(text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_guide_has_a_description_and_a_body() {
        for (name, text) in SKILLS {
            assert!(!description(text).is_empty(), "{name} has no description");
            assert!(
                get(name).unwrap().starts_with("# "),
                "{name} body starts with its title"
            );
        }
        assert!(list().contains("testing"));
        assert_eq!(get("nope"), None);
    }
}
