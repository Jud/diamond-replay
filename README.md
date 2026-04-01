# diamond-replay

A baseball game replay engine that turns play-by-play event streams into real statistics. Feed it scoring data, get back AVG, OBP, SLG, wOBA, FIP, ERA, and 40+ other stats per player.

Built for youth baseball. Tested against 11 real games.

## Quick start

```rust
let result = diamond_replay::replay_from_json(&event_json)?;

// Linescores
println!("Away: {:?}", result.linescore_away); // [3, 0, 2, 0, 0]
println!("Home: {:?}", result.linescore_home); // [0, 0, 0, 7]

// Per-player batting
for (id, player) in &result.player_stats {
    let b = &player.batting;
    println!("{id}: {}/{} ({:.3}) | {} RBI | {:.3} OBP | {:.3} SLG | {:.3} wOBA",
        b.hits.unwrap_or(0),
        b.ab.unwrap_or(0),
        b.avg.unwrap_or(0.0),
        b.rbi,
        b.obp.unwrap_or(0.0),
        b.slg.unwrap_or(0.0),
        b.woba.unwrap_or(0.0),
    );
}

// Per-player pitching
for (id, player) in &result.player_stats {
    if let Some(p) = &player.pitching {
        println!("{id}: {} IP | {:.2} ERA | {:.2} FIP | {:.1}% K | {:.1}% CSW",
            p.ip_display.as_deref().unwrap_or("0.0"),
            p.era.unwrap_or(0.0),
            p.fip.unwrap_or(0.0),
            p.k_pct.unwrap_or(0.0) * 100.0,
            p.csw_pct.unwrap_or(0.0) * 100.0,
        );
    }
}
```

## What you get

### Batting (per-player and team-level)

| Stat | Description |
|------|-------------|
| PA, AB, H, TB, XBH | Plate appearances, at-bats, hits, total bases, extra-base hits |
| AVG, OBP, SLG, OPS | The traditional slash line |
| ISO, BABIP | Isolated power, batting average on balls in play |
| wOBA | Weighted on-base average (the gold standard offensive metric) |
| K%, BB%, BB/K | Strikeout rate, walk rate, walk-to-K ratio |
| GB%, FB%, LD% | Ground ball, fly ball, line drive rates |
| HR/FB | Home run to fly ball rate |
| RBI, R, SB, CS, SB% | Runs batted in, runs, stolen bases, caught stealing |
| GIDP | Grounded into double play |
| QAB, QAB% | Quality at-bats (the #1 youth baseball process metric) |
| Competitive AB% | Plate appearances reaching a 2-strike count |
| P/PA | Pitches per plate appearance |
| Hard Hit% | Hard ground balls + line drives / balls in play |

### Pitching (per-player and team-level)

| Stat | Description |
|------|-------------|
| IP, BF, Pitches | Innings pitched, batters faced, pitch count |
| ERA | Earned run average (with error-tagged runner tracking) |
| FIP | Fielding independent pitching |
| WHIP | Walks + hits per inning pitched |
| K/9, BB/9, H/9, HR/9 | Rate stats per 9 innings |
| K%, BB%, K-BB% | Strikeout rate, walk rate, and the difference |
| SwStr% | Swinging strike rate |
| CSW% | Called strikes + whiffs rate (best K predictor) |
| FPS% | First pitch strike rate |
| CStr%, Foul% | Called strike rate, foul ball rate |
| BABIP | Batting average on balls in play (against) |
| HR/FB, GB%, FB%, LD% | Batted ball profile |
| Game Score | Bill James game score for the start |
| Pitches/IP | Pitch efficiency |

### Game data

| Field | Description |
|-------|-------------|
| Linescores | Runs per inning, home and away |
| Transition gaps | Dead time between half-innings (seconds) |
| Dead time per inning | Total non-play time per full inning |
| Timestamps | First and last event timestamps |

## How it works

The engine replays every pitch, every batted ball, every stolen base, reconstructing the full game state from a sequence of scoring events. It applies the rules of baseball: runners advance on hits, force on walks, tag on fly outs. Explicit base-running events from the scorer override the defaults when something unusual happens.

Every run is attributed to a specific player. Every out is tracked against the pitcher on the mound. The engine handles the mess that real scorers create: undo corrections, manual score overrides, dropped third strikes, catcher interference, short lineups with batting-order wrap, and scorer-entered totals that contradict the play-by-play.

After replay, a pure computation layer derives all rate statistics from the raw counts.

## Install

```toml
[dependencies]
diamond-replay = { git = "https://github.com/Jud/diamond-replay" }
```

## Input format

JSON arrays of scoring events. Each event has a `sequence_number`, an `event_data` JSON string containing the play details, and optional timestamps.

Events can be single plays or bundled transactions (e.g., a pitch + ball-in-play + base-running result in one atomic group). See `testdata/` for 11 complete game event streams.

## Test

```
cargo test
```

48 tests: 31 stat computation unit tests, 5 undo-resolution unit tests, 12 full-game integration tests verified against ground-truth linescores with per-player invariant checks (PA decomposition, hits decomposition, run attribution).

## Architecture

~3,500 lines of Rust. Three dependencies: `serde`, `serde_json`, `thiserror`.

```
src/
  lib.rs        public API: replay(), replay_from_json()
  event.rs      JSON parsing, typed enums for all event codes
  undo.rs       stack-based undo resolution
  state.rs      GameState, BaseState, BaseOccupant, PAContext
  replay.rs     the state machine: event loop, per-event handlers
  compute.rs    pure stat formulas: AVG, OBP, SLG, wOBA, FIP, ERA, CSW%, etc.
  score.rs      run recording, walk force-advance, score overrides
  player.rs     lineup tracking, per-player stat attribution, team aggregation
```

Pedantic clippy. Zero suppressions. No unsafe. No async.

## What we can't compute (yet)

These require tracking hardware that youth fields don't have:

Exit velocity, launch angle, barrel%, sprint speed, bat speed, Stuff+, xBA/xSLG/xwOBA, spin rate, pitch movement, OAA fielding, catcher framing.

See `docs/STATISTICS.md` for the full stat reference and `docs/EMERGING_STATS.md` for the analytics frontier.

## License

MIT
