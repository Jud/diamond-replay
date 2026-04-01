/// Raw per-half-inning stat counters.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RawStats {
    pub pitches: i32,
    pub balls: i32,
    pub strikes_swinging: i32,
    pub strikes_looking: i32,
    pub fouls: i32,
    pub bip: i32,
    pub hbp: i32,
    pub k: i32,
    pub k_looking: i32,
    pub k_swinging: i32,
    pub bb: i32,
    pub pa: i32,
    pub sb: i32,
    pub pb: i32,
    pub wp: i32,
    pub cs: i32,
    pub steals_of_home: i32,
    pub runs_on_bip: i32,
    pub runs_passive: i32,
    pub pitches_between_bip: Vec<i32>,
}

impl RawStats {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn merge(&mut self, other: &RawStats) {
        self.pitches += other.pitches;
        self.balls += other.balls;
        self.strikes_swinging += other.strikes_swinging;
        self.strikes_looking += other.strikes_looking;
        self.fouls += other.fouls;
        self.bip += other.bip;
        self.hbp += other.hbp;
        self.k += other.k;
        self.k_looking += other.k_looking;
        self.k_swinging += other.k_swinging;
        self.bb += other.bb;
        self.pa += other.pa;
        self.sb += other.sb;
        self.pb += other.pb;
        self.wp += other.wp;
        self.cs += other.cs;
        self.steals_of_home += other.steals_of_home;
        self.runs_on_bip += other.runs_on_bip;
        self.runs_passive += other.runs_passive;
        self.pitches_between_bip
            .extend_from_slice(&other.pitches_between_bip);
    }

    #[must_use]
    pub fn total_runs(&self) -> i32 {
        self.runs_on_bip + self.runs_passive
    }
}
