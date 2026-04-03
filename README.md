# diamond-replay

Replay engine for baseball play-by-play event streams. 40+ stats per player, plus youth-specific analytics. Built for youth baseball.

## CLI

```
diamond-replay game.json
diamond-replay game.json --json
diamond-replay game.json --json --little-league
diamond-replay game.json --json --no-steal-home
```

Four TUI views: Box Score, Batting, Pitching, Little League. Press `?` on any stat column for an interactive help card with formula, MLB benchmarks, youth context, and caveats.

`--little-league` adds per-team youth stats: run sourcing, pace, baserunning chaos, free bases.

`--no-steal-home` replays the game with steal-of-home attempts blocked. Runners stay at 3B but can still score on hits, walks, WP, and PB.

## Library

```rust
let result = diamond_replay::replay_from_json(&event_json)?;

for (id, player) in &result.player_stats {
    let b = &player.batting;
    println!("{id}: {}/{} | {:.3} wOBA", b.hits, b.ab, b.woba.unwrap_or(0.0));
}
```

## Stats

### Batting

PA, AB, H, TB, XBH, AVG, OBP, SLG, OPS, ISO, BABIP, wOBA, K%, BB%, BB/K, GB%, FB%, LD%, HR/FB, RBI, R, SB, CS, SB%, GIDP, QAB%, Competitive AB%, P/PA, Hard Hit%.

### Pitching

IP, BF, Pitches, ERA, FIP, WHIP, K/9, BB/9, H/9, HR/9, K%, BB%, K-BB%, SwStr%, CSW%, FPS%, CStr%, Foul%, BABIP, HR/FB, GB%, FB%, LD%, Game Score, Pitches/IP.

### Little League (team-level)

| Category | Stats |
|----------|-------|
| Run sourcing | Runs on BIP, passive runs, BIP run % |
| Pace | Pitches per BIP, median pitches between BIP |
| Baserunning | Steals of home, WP, PB, CS |
| Free bases | BB + HBP + WP + PB + SB, per inning |
| Pitching | Pitches, ball%, strike%, K/inn, BB/inn, BIP/inn |
| Defense | Opponent SB, free bases allowed per inning |

### Game data

Linescores, transition gaps, dead time per inning, timestamps.

## Install

```toml
[dependencies]
diamond-replay = { git = "https://github.com/Jud/diamond-replay" }
```

## Input format

JSON arrays of scoring events with `sequence_number`, `event_data` (JSON string), and optional timestamps. See `testdata/` for 14 complete game files.

## Test

```
cargo test
```

68 tests: 39 unit (stat computation, stat help coverage), 29 integration (full-game linescores, LL balance invariants, undo/redo, simulation).

## Architecture

~5,500 lines of Rust. Dependencies: `serde`, `serde_json`, `thiserror`, `ratatui`, `crossterm`.

```
src/
  lib.rs              public API
  event.rs            JSON parsing, typed event enums
  undo.rs             stack-based undo/redo resolution
  filter.rs           EventFilter trait, simulation filters
  state.rs            GameState, BaseState, PAContext
  replay.rs           state machine, event loop, LL stats
  compute.rs          pure stat formulas
  score.rs            run recording, force-advance, overrides
  player.rs           lineup tracking, stat attribution
  stat_help.rs        interactive stat help data (30 entries)
  bin/diamond-replay  TUI + JSON CLI
```

## Not computable

Requires tracking hardware: exit velocity, launch angle, barrel%, sprint speed, bat speed, Stuff+, xBA/xSLG/xwOBA, spin rate, pitch movement, OAA, catcher framing.

See `docs/STATISTICS.md` for full stat reference.

## License

MIT
