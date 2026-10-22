//! What a provider offers: its models, each with the reasoning levels it takes. A chat model is
//! built from a [`Selection`], which can only hold a level its model offers: there is no way to
//! construct one with another, so an unsupported level never reaches a request.

/// How much a model reasons before it answers, lowest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Effort {
    /// No reasoning: the fastest turn.
    Off,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
    Ultra,
}

impl Effort {
    pub const ALL: [Effort; 8] = [
        Effort::Off,
        Effort::Minimal,
        Effort::Low,
        Effort::Medium,
        Effort::High,
        Effort::XHigh,
        Effort::Max,
        Effort::Ultra,
    ];

    /// The name a person types and a provider's wire mostly uses.
    pub fn name(self) -> &'static str {
        match self {
            Effort::Off => "off",
            Effort::Minimal => "minimal",
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::XHigh => "xhigh",
            Effort::Max => "max",
            Effort::Ultra => "ultra",
        }
    }

    /// The level `name` names (`none` is `off`).
    pub fn named(name: &str) -> Option<Effort> {
        let name = name.trim().to_ascii_lowercase();
        if name == "none" {
            return Some(Effort::Off);
        }
        Effort::ALL.into_iter().find(|effort| effort.name() == name)
    }

    /// One line on what the level is for, for a picker.
    pub fn describe(self) -> &'static str {
        match self {
            Effort::Off => "no reasoning: the fastest turns",
            Effort::Minimal => "the least reasoning the model allows",
            Effort::Low => "fast, light reasoning",
            Effort::Medium => "balanced",
            Effort::High => "deep reasoning",
            Effort::XHigh => "deeper still, where the model has it",
            Effort::Max => "the most the model offers",
            Effort::Ultra => "beyond max, where the model has it",
        }
    }
}

/// One model a provider offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelEntry {
    /// What requests name it by.
    pub id: String,
    /// What a person reads.
    pub name: String,
    /// The levels it takes, lowest first; empty means it is not asked for any (`Off`).
    pub efforts: Vec<Effort>,
    /// The level it runs at when none was chosen.
    pub default: Effort,
}

impl ModelEntry {
    /// The levels it offers: [`Effort::Off`] alone when it lists none.
    pub fn offered(&self) -> &[Effort] {
        match self.efforts.is_empty() {
            true => &[Effort::Off],
            false => &self.efforts,
        }
    }
}

/// A model and a level it offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    model: ModelEntry,
    effort: Effort,
}

impl Selection {
    /// `model` at its default level, or its lowest when the default is not one it offers.
    pub fn new(model: ModelEntry) -> Self {
        let offered = model.offered();
        let effort = match offered.contains(&model.default) {
            true => model.default,
            false => offered[0],
        };
        Self { model, effort }
    }

    /// The same model at `effort`; `None` when the model does not offer it.
    pub fn with_effort(&self, effort: Effort) -> Option<Self> {
        self.model.offered().contains(&effort).then(|| Self {
            model: self.model.clone(),
            effort,
        })
    }

    pub fn model(&self) -> &ModelEntry {
        &self.model
    }

    pub fn effort(&self) -> Effort {
        self.effort
    }
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
