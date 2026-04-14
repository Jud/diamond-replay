use std::collections::HashSet;

/// Occupant of a base: either a known player or anonymous.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseOccupant {
    Player(String),
    Anonymous,
}

impl BaseOccupant {
    #[must_use]
    pub fn is_player(&self, id: &str) -> bool {
        matches!(self, Self::Player(pid) if pid == id)
    }

    #[must_use]
    pub fn is_anonymous(&self) -> bool {
        matches!(self, Self::Anonymous)
    }
}

/// State of the three bases.
#[derive(Debug, Clone)]
pub struct BaseState {
    bases: [Option<BaseOccupant>; 3], // index 0=1B, 1=2B, 2=3B
}

impl Default for BaseState {
    fn default() -> Self {
        Self::new()
    }
}

impl BaseState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            bases: [None, None, None],
        }
    }

    /// # Panics
    ///
    /// Panics if `base` is not in the range 1..=3.
    #[must_use]
    pub fn get(&self, base: usize) -> &Option<BaseOccupant> {
        assert!((1..=3).contains(&base), "base must be 1-3");
        &self.bases[base - 1]
    }

    /// # Panics
    ///
    /// Panics if `base` is not in the range 1..=3.
    pub fn set(&mut self, base: usize, occupant: Option<BaseOccupant>) {
        assert!((1..=3).contains(&base), "base must be 1-3");
        self.bases[base - 1] = occupant;
    }

    #[must_use]
    pub fn is_occupied(&self, base: usize) -> bool {
        self.get(base).is_some()
    }

    pub fn clear_all(&mut self) {
        self.bases = [None, None, None];
    }

    /// Move the occupant from one base to another.
    ///
    /// # Panics
    ///
    /// Panics if `from` or `to` is not in the range 1..=3.
    pub fn advance(&mut self, from: usize, to: usize) {
        let occ = self.get(from).clone();
        self.set(from, None);
        self.set(to, occ);
    }

    /// Clear a runner by ID. Returns true if found.
    pub fn clear_by_id(&mut self, runner_id: &str) -> bool {
        for b in 0..3 {
            if let Some(occ) = &self.bases[b] {
                if occ.is_player(runner_id) {
                    self.bases[b] = None;
                    return true;
                }
            }
        }
        false
    }

    /// Find which base (1-3) a player occupies by their ID.
    #[must_use]
    pub fn find_by_id(&self, id: &str) -> Option<usize> {
        for b in 0..3 {
            if let Some(occ) = &self.bases[b] {
                if occ.is_player(id) {
                    return Some(b + 1);
                }
            }
        }
        None
    }

    /// Clear a runner from the expected origin base.
    /// Two-pass: first prefers an Anonymous occupant, then falls back to any occupant.
    pub fn clear_fallback(&mut self, dest_base: usize) -> bool {
        if !(2..=4).contains(&dest_base) {
            return false;
        }
        let origin = dest_base - 1;
        let mut search = vec![origin];
        for b in [3, 2, 1] {
            if b != origin {
                search.push(b);
            }
        }
        // Pass 1: prefer Anonymous occupant
        for &b in &search {
            if (1..=3).contains(&b)
                && self.bases[b - 1]
                    .as_ref()
                    .is_some_and(BaseOccupant::is_anonymous)
            {
                self.bases[b - 1] = None;
                return true;
            }
        }
        // Pass 2: any occupant
        for &b in &search {
            if (1..=3).contains(&b) && self.bases[b - 1].is_some() {
                self.bases[b - 1] = None;
                return true;
            }
        }
        false
    }

    /// Clear a runner by ID, with anonymous fallback.
    pub fn clear_runner(&mut self, runner_id: &str, dest_base: usize) -> bool {
        if self.clear_by_id(runner_id) {
            return true;
        }
        self.clear_fallback(dest_base)
    }

    /// Replace one runner with another on the bases (player substitution).
    pub fn substitute_runner(&mut self, old_id: &str, new_id: &str) {
        for occ in &mut self.bases {
            if let Some(BaseOccupant::Player(pid)) = occ {
                if pid == old_id {
                    *pid = new_id.to_string();
                }
            }
        }
    }
}

/// Tracks which runners were auto-scored during eager auto-advance.
#[derive(Debug, Clone, Default)]
pub struct AutoAdvanceRecord {
    /// Player IDs of runners who were auto-scored (None for anonymous runners).
    pub scored: Vec<Option<String>>,
    /// Player IDs of auto-scored runners who were error-tagged at the time they scored.
    pub error_tagged: HashSet<String>,
}

/// Per-plate-appearance transient context for tracking pitch-level stats.
#[derive(Debug, Clone, Default)]
pub struct PAContext {
    pub pitches_in_pa: i32,
    pub reached_two_strikes: bool,
    pub first_pitch_strike: bool,
    pub pitches_after_two_strikes: i32,
}

/// Core mutable game state.
#[derive(Debug)]
pub struct GameState {
    pub home_id: String,
    pub away_id: String,
    pub offense: String,
    pub half_inning: usize,
    pub outs: i32,
    pub ball_count: i32,
    pub strike_count: i32,
    pub last_strike_type: Option<String>, // "strike_swinging" or "strike_looking"
    pub bases: BaseState,
    pub auto_advance: Option<AutoAdvanceRecord>,
    pub pa_context: PAContext,
    pub error_runners: HashSet<String>,
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

impl GameState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            home_id: String::new(),
            away_id: String::new(),
            offense: String::new(),
            half_inning: 0,
            outs: 0,
            ball_count: 0,
            strike_count: 0,
            last_strike_type: None,
            bases: BaseState::new(),
            auto_advance: None,
            pa_context: PAContext::default(),
            error_runners: HashSet::new(),
        }
    }

    pub fn reset_count(&mut self) {
        self.ball_count = 0;
        self.strike_count = 0;
        self.last_strike_type = None;
        self.pa_context = PAContext::default();
    }

    pub fn do_switch(&mut self) {
        self.half_inning += 1;
        self.offense = if self.offense == self.away_id {
            self.home_id.clone()
        } else {
            self.away_id.clone()
        };
        self.outs = 0;
        self.reset_count();
        self.bases.clear_all();
        self.auto_advance = None;
        self.error_runners.clear();
    }

    #[must_use]
    pub fn teams_set(&self) -> bool {
        !self.home_id.is_empty()
    }
}
