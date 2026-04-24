/// Rule policy used to compile a replay scenario before the core engine runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuleSet {
    /// Ground-truth replay rules.
    #[default]
    Standard,
    /// Simulation rules that keep runners from scoring on chaos advances.
    NoStealHome,
}

impl RuleSet {
    /// Ground-truth replay rules.
    pub const STANDARD: Self = Self::Standard;

    /// Simulation rules that keep runners from scoring on chaos advances.
    pub const NO_STEAL_HOME: Self = Self::NoStealHome;
}

/// Options controlling replay preprocessing and rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReplayOptions {
    pub rule_set: RuleSet,
}

impl ReplayOptions {
    /// Build options for ground-truth replay.
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            rule_set: RuleSet::STANDARD,
        }
    }

    /// Build options for the no-steal-home simulation.
    #[must_use]
    pub const fn no_steal_home() -> Self {
        Self {
            rule_set: RuleSet::NO_STEAL_HOME,
        }
    }
}
