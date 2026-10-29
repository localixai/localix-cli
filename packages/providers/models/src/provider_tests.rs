use super::*;

impl PartialEq for Provider {
    /// The same when they say the same: the models compared as they read, not as they are written.
    fn eq(&self, other: &Self) -> bool {
        (&self.name, &self.env) == (&other.name, &other.env) && self.models().eq(other.models())
    }
}
