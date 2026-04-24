# Legacy Statistics Reference

This documents every statistic computed by diamond-replay as of commit `2bc6f8c`
(the eager auto-advance refactor). Preserved as reference in case we need to
restore any of these during the stats rewrite.

---

## GameResult Output Shape

The `replay_game()` function returns a `GameResult` with these fields:

```rust
pub struct GameResult {
    pub home_id: String,
    pub away_id: String,
    pub linescore_away: Vec<i32>,       // runs per inning, away team
    pub linescore_home: Vec<i32>,       // runs per inning, home team
    pub away_batting: RawStats,         // merged half-inning stats, away
    pub home_batting: RawStats,         // merged half-inning stats, home
    pub away_halves_bat: i32,           // number of half-innings batted, away
    pub home_halves_bat: i32,           // number of half-innings batted, home
    pub first_timestamp: Option<i64>,   // earliest event timestamp (ms)
    pub last_timestamp: Option<i64>,    // latest event timestamp (ms)
    pub transition_gaps: Vec<f64>,      // seconds between consecutive halves
    pub dead_time_per_inning: Vec<f64>, // sum of top+bottom transition gaps per inning
    pub player_stats: HashMap<String, PlayerGameStats>,
}
```

---

## RawStats (Per-Half-Inning, Merged Into Per-Team)

Defined in `src/stats.rs`. Accumulated per half-inning, then merged into
`away_batting` and `home_batting` on the `GameResult`.

```rust
pub struct RawStats {
    pub pitches: i32,               // total pitches thrown
    pub balls: i32,                 // ball count pitches
    pub strikes_swinging: i32,      // swinging strikes
    pub strikes_looking: i32,       // called strikes
    pub fouls: i32,                 // foul balls
    pub bip: i32,                   // balls in play
    pub hbp: i32,                   // hit by pitch
    pub k: i32,                     // strikeouts (total)
    pub k_looking: i32,             // strikeouts looking
    pub k_swinging: i32,            // strikeouts swinging
    pub bb: i32,                    // walks
    pub pa: i32,                    // plate appearances
    pub sb: i32,                    // stolen bases
    pub pb: i32,                    // passed balls
    pub wp: i32,                    // wild pitches
    pub cs: i32,                    // caught stealing
    pub steals_of_home: i32,        // steals of home plate
    pub runs_on_bip: i32,           // runs scored on batted balls
    pub runs_passive: i32,          // runs scored via walks, WP, PB, errors, etc.
    pub pitches_between_bip: Vec<i32>, // pitch counts between each ball in play
}
```

### Derived values:
- `total_runs()` = `runs_on_bip + runs_passive`

### How each field is populated:

| Field | Incremented when |
|-------|-----------------|
| `pitches` | Every `pitch` event with `advancesCount=true` |
| `balls` | `pitch` result = `ball` |
| `strikes_swinging` | `pitch` result = `strike_swinging` |
| `strikes_looking` | `pitch` result = `strike_looking` |
| `fouls` | `pitch` result = `foul` |
| `bip` | `pitch` result = `ball_in_play` |
| `hbp` | `pitch` result = `hit_by_pitch`, or `end_at_bat` reason = `hit_by_pitch` |
| `ci` | `end_at_bat` reason = `catcher_interference` |
| `k` | Strike count reaches 3 (via strikes), or dropped third strike |
| `k_looking` | K where last strike was `strike_looking` |
| `k_swinging` | K where last strike was `strike_swinging` |
| `bb` | Ball count reaches 4 |
| `pa` | On K, BB, HBP, or BIP (each increments PA by 1) |
| `sb` | `base_running` playType = `stole_base` |
| `pb` | `base_running` playType = `passed_ball` |
| `wp` | `base_running` playType = `wild_pitch` |
| `cs` | `base_running` playType = `caught_stealing` |
| `steals_of_home` | SB, PB, or WP with `base=4` |
| `runs_on_bip` | `score_run()` called with `on_bip=true` (auto-advance scoring, HR scoring, explicit BR BIP scoring) |
| `runs_passive` | `score_run()` called with `on_bip=false` (walks, WP, PB, dropped third strike scoring) |
| `pitches_between_bip` | On each BIP, the current `pitches_since_last_bip` counter is pushed |

---

## Per-Player Stats

Defined in `src/player.rs`. Stored in `GameResult::player_stats` keyed by
player ID.

```rust
pub struct PlayerGameStats {
    pub player_id: String,
    pub team_id: String,
    pub batting: BattingStats,
    pub baserunning: BaserunningStats,
    pub pitching: Option<PitchingStats>,  // None if player didn't pitch
}
```

### BattingStats

```rust
pub struct BattingStats {
    pub pa: i32,           // plate appearances
    pub k: i32,            // strikeouts (total)
    pub k_looking: i32,    // strikeouts looking (called)
    pub k_swinging: i32,   // strikeouts swinging
    pub bb: i32,           // walks
    pub hbp: i32,          // hit by pitch
    pub ci: i32,           // catcher interference
    pub singles: i32,      // singles
    pub doubles: i32,      // doubles
    pub triples: i32,      // triples
    pub home_runs: i32,    // home runs
    pub sac_fly: i32,      // sacrifice flies
    pub sac_bunt: i32,     // sacrifice bunts
    pub fc: i32,           // fielder's choice
    pub roe: i32,          // reached on error
}
```

#### How each field is populated:

| Field | Recorded by | Trigger |
|-------|------------|---------|
| `pa` | `record_k`, `record_bb`, `record_hbp`, `record_ci`, `record_bip`, `record_dropped_k` | Every completed PA |
| `k` | `record_k`, `record_dropped_k` | Strikeout (3 strikes or dropped third) |
| `k_looking` | `record_k`, `record_dropped_k` | K where `looking=true` (last strike was called) |
| `k_swinging` | `record_k`, `record_dropped_k` | K where `looking=false` |
| `bb` | `record_bb` | Ball 4 |
| `hbp` | `record_hbp` | Hit by pitch |
| `ci` | `record_ci` | Catcher interference |
| `singles` | `record_bip` | `PlayResult::Single` |
| `doubles` | `record_bip` | `PlayResult::Double` |
| `triples` | `record_bip` | `PlayResult::Triple` |
| `home_runs` | `record_bip` | `PlayResult::HomeRun` |
| `sac_fly` | `record_bip` | `PlayResult::SacrificeFly` |
| `sac_bunt` | `record_bip` | `PlayResult::SacrificeBunt` |
| `fc` | `record_bip` | `PlayResult::FieldersChoice` |
| `roe` | `record_bip` | `PlayResult::Error` |

Note: `record_dropped_k` is functionally identical to `record_k` — both
increment pa, k, and k_looking/k_swinging. The separate method exists for
call-site clarity.

### BaserunningStats

```rust
pub struct BaserunningStats {
    pub runs: i32,   // runs scored (times crossing home plate)
    pub sb: i32,     // stolen bases
    pub cs: i32,     // caught stealing
}
```

| Field | Recorded by | Trigger |
|-------|------------|---------|
| `runs` | `record_run` | Runner crosses home (auto-advance, explicit BR base=4, HR scoring, walk/HBP force-advance) |
| `sb` | `record_sb` (via `record_baserunning`) | `base_running` playType = `stole_base` |
| `cs` | `record_cs` (via `record_baserunning`) | `base_running` playType = `caught_stealing` |

Additional methods:
- `undo_run(runner_id)` — decrements `runs` by 1 (used when a BR event undoes an auto-scored run)
- `adjust_team_runs(team_id, delta)` — bulk adjustment when a score override reduces a team's total

### PitchingStats

```rust
pub struct PitchingStats {
    pub pitches: i32,       // total pitches thrown
    pub balls: i32,         // ball pitches
    pub strikes: i32,       // strike pitches (swinging + looking + foul + BIP)
    pub k: i32,             // strikeouts
    pub bb: i32,            // walks allowed
    pub hbp: i32,           // hit batters
    pub hits_allowed: i32,  // total hits allowed (1B + 2B + 3B + HR)
    pub hr_allowed: i32,    // home runs allowed
    pub runs_allowed: i32,  // total runs allowed
}
```

| Field | Recorded by | Trigger |
|-------|------------|---------|
| `pitches` | `record_pitch_thrown` | Every pitch with `advancesCount=true` |
| `balls` | `record_pitch_thrown` | Pitch result = `ball` |
| `strikes` | `record_pitch_thrown` | Any non-ball pitch |
| `k` | `record_pitch_k` | Batter strikes out |
| `bb` | `record_pitch_bb` | Batter walks |
| `hbp` | `record_pitch_hbp` | Batter hit by pitch |
| `hits_allowed` | `record_pitch_hit` | BIP result is Single, Double, Triple, or HR |
| `hr_allowed` | `record_pitch_hit` | BIP result is HomeRun |
| `runs_allowed` | `record_pitch_run` | Any run scores while this pitcher is on the mound |

Additional method:
- `undo_pitch_run(defense_team)` — decrements current pitcher's `runs_allowed` by 1

---

## Scoring Module (src/score.rs)

### Functions

- `score_run(half_inning, runs_by_half, half_stats, on_bip)` — Increments
  run count for the half-inning. If `on_bip=true`, increments `runs_on_bip`;
  otherwise `runs_passive`.

- `undo_score_run(half_inning, runs_by_half, half_stats, on_bip)` — Reverses
  a prior `score_run`. Removes the half-inning entry from `runs_by_half` if
  count reaches 0.

- `force_advance_walk_score(half_inning, bases, runs_by_half, half_stats)` —
  If bases loaded, scores a run (passive). Returns true if scored.

- `apply_walk_bases(bases, batter_id)` — Performs force-advance base
  mutations for a walk/HBP. Moves runners up, places batter at 1B.

- `apply_score_override(half_inning, home_id, away_id, runs_by_half,
  half_stats, scores)` — Applies scorer-entered score corrections. Adjusts
  `runs_by_half` and `half_stats` to match the override values. Handles both
  positive (adding runs) and negative (removing runs) deltas.

- `ensure_stats(half_stats, half_inning)` — Grows the `half_stats` Vec to
  cover the given half-inning index.

### ScoreOverrideEntry

```rust
pub struct ScoreOverrideEntry {
    pub team_id: String,
    pub score: i32,
}
```

---

## Temporal Stats

Computed during aggregation in `replay.rs`:

- **transition_gaps**: For each consecutive pair of half-innings, the time
  (seconds) between the last event of one half and the first event of the next.
  Computed from `half_first_ts` and `half_last_ts` HashMaps.

- **dead_time_per_inning**: Pairs of consecutive transition gaps summed
  (top-to-bottom + bottom-to-next-top). Represents total dead time per full
  inning.

- **first_timestamp / last_timestamp**: Earliest and latest event timestamps
  across the entire game. Updated incrementally via `record_ts()`.

---

## State Tracking (src/state.rs)

### BaseState
Array of 3 `Option<BaseOccupant>` (index 0=1B, 1=2B, 2=3B).

```rust
pub enum BaseOccupant {
    Player(String),  // known player ID
    Anonymous,       // runner with unknown ID
}
```

Methods: `get(base)`, `set(base, occ)`, `is_occupied(base)`, `clear_all()`,
`advance(from, to)`, `clear_by_id(runner_id)`, `find_by_id(id)`,
`clear_fallback(dest_base)`, `clear_runner(runner_id, dest_base)`.

### AutoAdvanceRecord
Tracks runners auto-scored during eager auto-advance so BR events can detect
confirmations vs. corrections.

```rust
pub struct AutoAdvanceRecord {
    pub scored: Vec<Option<String>>,
}
```

### GameState
Core mutable game state during replay:

```rust
pub struct GameState {
    pub home_id: String,
    pub away_id: String,
    pub offense: String,          // team currently batting
    pub half_inning: usize,       // 0-indexed half-inning counter
    pub outs: i32,
    pub ball_count: i32,
    pub strike_count: i32,
    pub last_strike_type: Option<String>,  // "strike_swinging" or "strike_looking"
    pub pitches_since_last_bip: i32,
    pub bases: BaseState,
    pub auto_advance: Option<AutoAdvanceRecord>,
}
```

---

## Event Types Processed

| Event code | Handler | What it does |
|-----------|---------|-------------|
| `set_teams` | `handle_set_teams` | Sets home/away team IDs, offense starts as away |
| `fill_lineup_index` | `PlayerTracker::handle_fill_lineup` | Maps player to lineup slot |
| `fill_lineup` | `PlayerTracker::handle_fill_lineup_roster` | Maps player to next sequential slot |
| `fill_position` | `PlayerTracker::handle_fill_position` | Records player position, tracks pitcher |
| `goto_lineup_index` | `PlayerTracker::handle_goto` | Advances batting order to specific index |
| `pitch` | `handle_pitch` | Processes pitch result, updates count, triggers BIP |
| `ball_in_play` | `handle_ball_in_play` | Records hit type, applies eager auto-advance |
| `base_running` | `handle_base_running` | Moves/scores runners, handles outs, corrections |
| `end_at_bat` | `handle_end_at_bat` | Handles HBP and catcher interference end-of-AB |
| `end_half` | (inline) | Triggers half-inning switch |
| `override` | `handle_override` | Applies score corrections, state overrides |

---

## Data Available But Not Used

These fields exist in the GC event stream and are parsed or available but not
stored in any stats:

- `defenders` array on `ball_in_play` — position + x,y coordinates of fielders
- `defenders` array on `base_running` — fielder positions on the play
- `hrLocation` on `ball_in_play` — x,y landing coordinates for home runs
- `extendedPlayResult` on `ball_in_play` — detailed play classification
- `pitchSpeedProvider` on `pitch` — pitch speed data source
- `intentional` on `end_at_bat` — intentional walk/HBP flag
- `advancesRunners` on `pitch` — parsed but unused boolean
- `pitcher_decision` events — win/loss/save decisions (not processed)
- `playFlavor` on `base_running` — e.g. "on_the_throw" (parsed in attrs but not stored)
- `BipPlayType` (ground_ball, fly_ball, line_drive, pop_fly, hard_ground_ball) — parsed per-event but NOT stored per-player
