# What We Learned Building diamond-replay

Everything we figured out building a baseball replay engine from GameChanger event streams. These aren't all proven facts — some are theories based on observed behavior. Labeled accordingly.

## How GameChanger Scoring Works

### The event stream is a recording of what the scorer tapped (proven)

Every event in the stream maps to a specific action the scorer took in the GC mobile app. A `pitch` event = the scorer tapped a pitch result. A `ball_in_play` event = they selected the hit type and fielder. A `base_running` event = they manually moved a runner.

### Explicit base_running events are corrections, not the full picture (proven)

GC auto-advances runners after a ball in play. If the auto-advance was correct, NO `base_running` event is emitted. Events only appear when the scorer CORRECTS the auto-advance — dragging a runner to a different base or tapping "Back" to undo an auto-score.

**Implication:** Absence of a `base_running` event means the default was accepted. The replay engine must apply the same defaults as GC and only override when explicit events say otherwise.

### GC auto-advance rules (verified against 11 real games)

| Hit Type | Runner on 1B | Runner on 2B | Runner on 3B |
|----------|-------------|-------------|-------------|
| Single   | → 2B        | → 3B        | → Scores    |
| Double   | → 3B        | → Scores    | → Scores    |
| Triple   | → Scores    | → Scores    | → Scores    |
| Home Run | → Scores    | → Scores    | → Scores    |

- Singles: runner from 3B ALWAYS scores, regardless of fly ball or ground ball type. Tested: removing the fly/ground distinction passed all 7 original games. Adding it back broke a new game.
- Doubles: runner from 1B goes to 3B, NOT home. Tested: changing to "advance 2 bases" (1B→scores) broke the Phillies/Cardinals game.

### advancesCount=False pitches (proven)

Some pitches have `advancesCount: false`. The pitch still happened but shouldn't advance the ball/strike count. Seen with:
- Dropped third strikes: the 3rd strike was already counted, the pitch is just the vehicle for the `ball_in_play dropped_third_strike` event
- Hit by pitch: the ball that hit the batter, followed by `end_at_bat reason: hit_by_pitch`
- Catcher interference: same pattern, followed by `end_at_bat reason: catcher_interference`

The engine must still record the strike type (looking/swinging) from these pitches for K attribution, even though it skips the count.

### ~30-50% of batters never appear in any base_running event (observed)

Many players reach base, advance, and score entirely through auto-advance. They're never referenced by `runnerId` in any `base_running` event. Their identity on base comes from the lineup tracker knowing who batted.

### The scorer can credit a different player than expected (theory)

When a scorer explicitly records `advanced_on_last_play base=4` for a specific player, that player may be tracked at a different base in our engine than where they physically were in the game. The scorer knows who actually scored; our base tracking may disagree because the player was placed by `batter_occupant` at their hit destination (e.g., 1B for a single) but physically advanced further via auto-advance that wasn't explicitly recorded.

## Event Types

### Ball in play results

Batter out: `batter_out`, `batter_out_advance_runners`, `infield_fly`, `dropped_third_strike_batter_out`, `sacrifice_fly`, `sacrifice_bunt`, `ground_out`, `fly_out`, `line_out`, `pop_out`, `double_play`

Batter reaches: `single`, `double`, `triple`, `home_run`, `fielders_choice`, `error`, `dropped_third_strike`

### Base running play types

Movement: `stole_base`, `passed_ball`, `wild_pitch`, `advanced_on_last_play`, `advanced_on_error`, `on_same_error`, `on_same_pitch`, `defensive_indifference`, `other_advance`

Outs: `caught_stealing`, `out_on_last_play`, `picked_off`

Stayed: `remained_on_last_play`, `attempted_pickoff`

### End at bat reasons

- `hit_by_pitch`: batter awarded 1B, force advance
- `catcher_interference`: same as HBP for base placement

### Lineup events

- `fill_lineup_index`: explicit batting slot assignment (most common)
- `fill_lineup`: no index — events arrive in batting order, assign sequential slots
- `goto_lineup_index`: sparse corrections/lineup wrap
- `confirm_end_of_lineup`: batting order wrapped around

### Override events

Can set: half-inning (top/bottom), out count, ball/strike count, team score totals. Score overrides are the scorer manually entering the "correct" total when the play-by-play disagrees.

## Engine Architecture Decisions

### Movement log over occupant comparison (Codex-recommended)

`already_handled` in the resolver used to compare base occupants (snapshot vs current) to detect runner movement. This broke when we replaced Anonymous runners with Player(batter_id). The fix: track explicit movements in a `handled_bases: HashSet<usize>` set. No occupant comparison.

Codex evaluated 5 approaches and ranked this #1. The other contenders:
- Dual identity (custom PartialEq) — ranked #2, clever but fragile
- Parallel tracker — ranked #3, two sources of truth
- Post-hoc tagging — ranked #4, lineup math is brittle
- Minimal rewrite (is_none check) — ranked #5, misses refilled bases

### Every runner should have a name (proven)

The `batter_occupant()` function places `Player(batter_id)` on base instead of `Anonymous`. The batter's ID is captured from the lineup tracker before `record_*` methods advance the batting order. Anonymous only fires if the lineup tracker has zero entries for a team (never happens in real games — verified across all 11 test games).

A batter can be on base AND bat again (6-player lineup wrap). That's fine — they're still the same person and get credit for both.

### Score override reconciliation (proven)

When a scorer override reduces a team's total, the player-level run totals must be adjusted to match. `adjust_team_runs` removes runs from players with the most runs first. This matches the Python engine's `_apply_score_override` behavior.

## Known Open Issue

### clear_runner finds runner at wrong base (3 McCabe games)

When `clear_runner` finds a player by ID at base 1 (placed by batter_occupant) but they're scoring from what should be base 3, `handled_bases` marks base 1 instead of base 3. The implicit resolve then also scores from base 3 — double count.

The Python engine avoids this because Anonymous runners use positional fallback (clear from 3B first). The Rust engine finds by ID and clears the wrong base.

Fix: make `clear_runner` return `Option<usize>` (which base was actually cleared) and use THAT for `handled_bases`. See `DEBUG_MCCABE_FAILURES.md` for the full analysis.
