# Debugging the 3 McCabe Game Failures

## Status

3 of 4 new McCabe games fail linescore tests. The Reds game passes. All original 7 games pass.

```
McCabe_Tigers_Mets:    got A=[0,0,0,4] expected A=[0,0,0,3]  — extra run
McCabe_Tigers_Yankees: got A=[0,4,5,3] expected A=[1,3,5,3]  — run shifted between innings
McCabe_Tigers_Angels:  got A=[1,4,3,1] expected A=[1,5,3,1]  — missing run
```

## Root Cause (Proven)

The Python engine gets the correct linescores. The difference is in how `clear_runner` interacts with `handled_bases` when a scorer explicitly scores a runner via `base_running advanced_on_last_play base=4`.

### The Scenario (McCabe_Tigers_Mets, 4th inning)

1. seq=195: single (ground ball). PendingImplicit set. Snapshot has runner at 3B.
2. seq=196: explicit `base_running advanced_on_last_play base=4 runner=9b5424e3`
   - The scorer explicitly credits 9b5424e3 with scoring
   - `find_by_id(9b5424e3)` in the snapshot finds them at BASE 1 (placed by `batter_occupant` from a prior resolve)
   - `handled_bases.insert(1)` — marks base 1 as handled
   - `clear_runner(9b5424e3, 4)` clears base 1 by ID
3. seq=198: pitch resolves the pending from seq=195
   - `already_handled(3)`: runner at 3B, base 3 NOT in handled_bases → NOT handled → scores!
   - But this runner SHOULD NOT score — the explicit BR at seq=196 already credited the scoring play

**Result: double-counted run. 4 runs instead of 3.**

### Why Python Gets It Right

Python uses `Anonymous` on bases. When seq=196 fires:
- `_clear_runner(9b5424e3, 4)`: can't find by ID (bases have `True`/Anonymous)
- Fallback: dest=4, origin=3. Clears the Anonymous at base 3
- Base 3 is now empty
- When resolve fires: `current.get(3) != snapshot.get(3)` → empty ≠ Anonymous → "handled" → skip

The Python fallback clears the RIGHT base (3, nearest to home) because it doesn't know IDs. The Rust engine finds the runner by ID at base 1, clears the WRONG base.

### The Fix Needed

The `handled_bases` recording must mark the correct base. Options:

**Option A**: Change `clear_runner` for dest=4 to return which base was actually cleared. Use THAT base for `handled_bases`, not the `find_by_id` result from the snapshot.

**Option B**: When recording handled_bases for a scoring play (base=4), if `find_by_id` finds the runner at a base far from home (base 1 or 2), ALSO mark the highest occupied snapshot base as handled. But this was tried and broke the Reds game (too aggressive — it suppressed valid scores).

**Option C**: For dest=4, skip `find_by_id` and always use the positional fallback (clear from 3B first). This matches the Python behavior but loses the ability to clear the specific player by ID. The `handled_bases` would then get the correct base (3).

**Option D**: Make `clear_runner` return `Option<usize>` (the base that was cleared) instead of `bool`. Then `handle_base_running` records THAT base in `handled_bases`. This is the cleanest — the handled base is always the one that was actually cleared, not a guess.

### Recommendation

Option D. Change `clear_runner` return type, update all call sites. The `handled_bases` becomes: "which base did clear_runner actually clear?" This is always correct regardless of where `find_by_id` found the runner.

## Theories About GameChanger Scoring App Behavior

Understanding these informs how to interpret the event stream:

### Theory 1: Events Are Literally What The Scorer Tapped

The event stream is a recording of scorer actions, not a computed game state. When the scorer taps "Ball In Play → Single," GC auto-advances runners. When the scorer sees something wrong, they tap a runner and drag them — that produces a `base_running` event.

### Theory 2: Explicit BR Events Are Corrections To Auto-Advance

If GC's auto-advance was correct, NO explicit `base_running` event is emitted. An event only appears when the scorer CORRECTS the auto-advance. This means: absence of a BR event = the default was accepted.

### Theory 3: GC Auto-Advance Rules (Verified)

- Single: all runners advance 1 base. Runner from 3B ALWAYS scores (no fly/ground distinction).
- Double: runners from 2B and 3B score. Runner from 1B goes to 3B (NOT home).
- Triple: everyone scores.

### Theory 4: Some Runners Are Never Referenced By runnerId

About 30-50% of batters never appear in any `base_running` event. They exist on base purely through auto-advance. Their identity is known from the lineup tracker + batting order position.

### Theory 5: The Scorer Can Credit A Different Player Than Expected

When the scorer explicitly records a run via `advanced_on_last_play base=4`, they may credit it to a player who our engine has tracked at a different base than where the "real" runner was. The scorer knows who scored in the physical game; our engine's base tracking may disagree.

### Theory 6: `fill_lineup` vs `fill_lineup_index`

Some scorers use `fill_lineup` (no index) instead of `fill_lineup_index`. Events arrive in batting order — assign sequential indices. Both map player→team. Fixed in current code.

### Theory 7: `other_advance` and `catcher_interference`

- `other_advance`: equivalent to `advanced_on_last_play` (runner advancement, BIP scoring)
- `catcher_interference`: equivalent to `hit_by_pitch` for base placement (batter awarded 1B, force advance)

Both are handled in the current code.

## Cross-Reference

- Python engine: `/Users/jud/ENA/compute_game_stats.py` (gets correct linescores for all games)
- GC API docs: `/Users/jud/ENA/GAMECHANGER_API.md` (includes scorer app behavior section)
- Test data: `testdata/McCabe_Tigers_{Mets,Yankees,Angels}.json`
- Box scores: `testdata/box_scores.json`
