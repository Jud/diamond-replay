# ◇ diamond-replay

A stateful baseball game replay engine. Feed it a scoring event stream, get back linescores, per-team stats, and per-player attribution — all from the raw play-by-play data.

Built for youth baseball. Tested against 7 real games. Every runner on every base has a name.

## What it does

```rust
let result = diamond_replay::replay_from_json(&event_stream)?;

// Per-inning linescores
result.linescore_away  // [3, 0, 2, 0, 0]
result.linescore_home  // [0, 0, 0, 7]

// Per-player stats
for (id, stats) in &result.player_stats {
    println!("{}: {}AB {}H {}R {}K",
        id,
        stats.batting.pa,
        stats.batting.singles + stats.batting.doubles
            + stats.batting.triples + stats.batting.home_runs,
        stats.baserunning.runs,
        stats.batting.k,
    );
}
```

The engine replays every pitch, every batted ball, every stolen base — reconstructing the full game state from a sequence of scoring events. It handles the mess that real scorers create: undo corrections, manual overrides, dropped third strikes, scorer-entered totals that don't match the play-by-play.

## The engine

The replay is a state machine. It tracks:

- **Ball/strike count** — walks, strikeouts, fouls, HBP
- **Outs** — batted-ball outs, caught stealing, picked off, dropped third strikes
- **Base runners** — who is where, by player ID (not anonymous placeholders)
- **Half-inning transitions** — 3 outs or explicit end-half events
- **Implicit runner advancement** — singles advance runners from 2nd, doubles score from 3rd, etc.
- **Scorer corrections** — undo events, score overrides, half-inning jumps

Every run scored is attributed to a specific player. The engine knows who walked, who singled, who stole home — and who scored on each play.

### How implicit advancement works

When a ball is put in play, the event stream tells you the outcome (single, double, error, sac fly) but not always where every runner ended up. The engine applies the rules of baseball:

- **Single (ground ball)**: runner from 3rd scores, others advance one base
- **Single (fly ball)**: runner from 3rd holds, others advance
- **Double**: runners from 2nd and 3rd score, runner from 1st to 3rd
- **Triple**: everyone scores
- **Sac fly**: runner from 3rd scores if someone is behind them

Explicit `base_running` events override the defaults. The engine uses a **movement log** to track which bases were explicitly handled, so it never double-advances a runner that was already moved by a scorer event.

## Input format

The engine consumes JSON arrays of raw scoring events. Each event has a `sequence_number`, an `event_data` JSON string containing the play details, and optional timestamps.

Events can be single plays or bundled transactions (e.g., a pitch + ball-in-play + base-running result in one atomic group). The engine handles both.

See `testdata/` for 7 complete game event streams.

## Output

```rust
pub struct GameResult {
    pub home_id: String,
    pub away_id: String,
    pub linescore_away: Vec<i32>,
    pub linescore_home: Vec<i32>,
    pub away_batting: RawStats,      // team-level: PA, K, BB, BIP, SB, WP, PB...
    pub home_batting: RawStats,
    pub away_halves_bat: i32,        // innings batted (5 for a 4.5-inning game)
    pub home_halves_bat: i32,        // innings batted (4 for a 4.5-inning game)
    pub first_timestamp: Option<i64>,
    pub last_timestamp: Option<i64>,
    pub transition_gaps: Vec<f64>,   // dead time between half-innings (seconds)
    pub dead_time_per_inning: Vec<f64>,
    pub player_stats: HashMap<String, PlayerGameStats>,
}
```

Per-player stats include batting (PA, K, BB, 1B/2B/3B/HR, SF, SH, FC, ROE), baserunning (R, SB, CS), and pitching (pitches, balls, strikes, K, BB, hits/runs allowed).

## Install

```toml
[dependencies]
diamond-replay = { git = "https://github.com/Jud/diamond-replay" }
```

## Test

```
cargo test
```

13 tests: 7 full-game integration tests verified against ground-truth box scores, 5 undo-resolution unit tests, 1 player-attribution test asserting per-player run totals match linescores across all games.

## Design

~2,200 lines of Rust. Three dependencies: `serde`, `serde_json`, `thiserror`.

```
src/
  lib.rs        — public API: replay(), replay_from_json()
  event.rs      — JSON parsing, typed enums for all event codes
  undo.rs       — stack-based undo resolution
  state.rs      — GameState, BaseState, BaseOccupant, PendingImplicit
  replay.rs     — the state machine: event loop + per-event handlers
  resolve.rs    — implicit runner advancement (the baseball rules)
  score.rs      — run recording, walk/HBP force-advance, score overrides
  stats.rs      — per-half-inning stat counters
  player.rs     — lineup tracking, per-player stat attribution
```

Pedantic clippy. Zero suppressions. No unsafe. No async.

## License

MIT
