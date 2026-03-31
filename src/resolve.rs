use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;

use crate::event::{BipCause, BipPlayType, PlayResult};
use crate::score;
use crate::state::{BaseOccupant, BaseState, PendingImplicit};
use crate::stats::RawStats;

/// Check if a runner at `base` in the snapshot was already handled
/// by an explicit `base_running` event (runner ID in `explicit`)
/// or by a `base_running` event that recorded the base in `handled_bases`.
fn already_handled<S: BuildHasher, S2: BuildHasher>(
    base: usize,
    snapshot: &BaseState,
    explicit: &HashSet<String, S>,
    handled_bases: &HashSet<usize, S2>,
) -> bool {
    match snapshot.get(base) {
        None => true,
        Some(BaseOccupant::Player(id)) => {
            explicit.contains(id.as_str()) || handled_bases.contains(&base)
        }
        Some(BaseOccupant::Anonymous) => handled_bases.contains(&base),
    }
}

// ---------------------------------------------------------------------------
// Per-play-result advancement helpers. Each returns the number of BIP runs.
// ---------------------------------------------------------------------------

struct Ctx<'a, S: BuildHasher, S2: BuildHasher, S3: BuildHasher> {
    hi: usize,
    snap: &'a BaseState,
    bases: &'a mut BaseState,
    explicit: &'a HashSet<String, S>,
    handled_bases: &'a HashSet<usize, S3>,
    batter_id: Option<&'a str>,
    runs: &'a mut HashMap<usize, i32, S2>,
    stats: &'a mut Vec<RawStats>,
    scored: Vec<Option<String>>,
}

impl<S: BuildHasher, S2: BuildHasher, S3: BuildHasher> Ctx<'_, S, S2, S3> {
    fn handled(&self, base: usize) -> bool {
        already_handled(base, self.snap, self.explicit, self.handled_bases)
    }

    /// Returns `Player(id)` if a batter ID is known and that player is NOT
    /// already occupying a base; otherwise returns `Anonymous`.
    fn batter_occupant(&self) -> BaseOccupant {
        if let Some(id) = self.batter_id {
            if self.bases.find_by_id(id).is_none() {
                return BaseOccupant::Player(id.to_string());
            }
        }
        BaseOccupant::Anonymous
    }

    fn score_bip(&mut self, base: usize) {
        score::score_run(self.hi, self.runs, self.stats, true);
        self.record_scorer(base);
    }

    fn score_passive(&mut self, base: usize) {
        score::score_run(self.hi, self.runs, self.stats, false);
        self.record_scorer(base);
    }

    fn record_scorer(&mut self, base: usize) {
        // Check current bases first (may have Player ID from prior resolve),
        // fall back to snapshot.
        let pid = match self.bases.get(base) {
            Some(BaseOccupant::Player(id)) => Some(id.clone()),
            _ => match self.snap.get(base) {
                Some(BaseOccupant::Player(id)) => Some(id.clone()),
                _ => None,
            },
        };
        self.scored.push(pid);
    }

    /// Score unhandled runner, clear their base.
    fn score_if_live(&mut self, base: usize) -> bool {
        if !self.handled(base) {
            self.score_bip(base);
            self.bases.set(base, None);
            return true;
        }
        false
    }

    /// Advance unhandled runner from `from` to `to`.
    fn advance_if_live(&mut self, from: usize, to: usize) {
        if !self.handled(from) {
            self.bases.set(from, None);
            self.bases.set(to, self.snap.get(from).clone());
        }
    }
}

fn resolve_triple(cx: &mut Ctx<'_, impl BuildHasher, impl BuildHasher, impl BuildHasher>) {
    for b in [1, 2, 3] {
        cx.score_if_live(b);
    }
    cx.bases.set(3, Some(cx.batter_occupant()));
    cx.bases.set(2, None);
    cx.bases.set(1, None);
}

fn resolve_double(cx: &mut Ctx<'_, impl BuildHasher, impl BuildHasher, impl BuildHasher>) {
    cx.score_if_live(3);
    cx.score_if_live(2);
    cx.advance_if_live(1, 3);
    cx.bases.set(2, Some(cx.batter_occupant()));
}

fn resolve_single(
    cx: &mut Ctx<'_, impl BuildHasher, impl BuildHasher, impl BuildHasher>,
    is_fly: bool,
) {
    if !is_fly {
        cx.score_if_live(3);
    }
    cx.advance_if_live(2, 3);
    cx.advance_if_live(1, 2);
    cx.bases.set(1, Some(cx.batter_occupant()));
}

fn resolve_advance_out(cx: &mut Ctx<'_, impl BuildHasher, impl BuildHasher, impl BuildHasher>) {
    let has_behind = cx.snap.is_occupied(1) || cx.snap.is_occupied(2);
    if has_behind {
        cx.score_if_live(3);
    }
    cx.advance_if_live(2, 3);
    cx.advance_if_live(1, 2);
}

fn resolve_fielders_choice(cx: &mut Ctx<'_, impl BuildHasher, impl BuildHasher, impl BuildHasher>) {
    cx.score_if_live(3);
    cx.advance_if_live(2, 3);
    cx.bases.set(1, Some(cx.batter_occupant()));
}

fn resolve_error(cx: &mut Ctx<'_, impl BuildHasher, impl BuildHasher, impl BuildHasher>) {
    cx.score_if_live(3);
    cx.advance_if_live(2, 3);
    cx.advance_if_live(1, 2);
    cx.bases.set(1, Some(cx.batter_occupant()));
}

fn resolve_dropped_third(
    cx: &mut Ctx<'_, impl BuildHasher, impl BuildHasher, impl BuildHasher>,
    cause: Option<BipCause>,
) {
    if cause.is_some_and(BipCause::is_ball_away) {
        // Wild pitch / passed ball: all runners advance one base
        if !cx.handled(3) {
            cx.score_passive(3);
            cx.bases.set(3, None);
        }
        cx.advance_if_live(2, 3);
        cx.advance_if_live(1, 2);
        cx.bases.set(1, Some(cx.batter_occupant()));
    } else {
        // Force-advance walk: if bases loaded, runner from 3B scores
        if cx.bases.is_occupied(1) && cx.bases.is_occupied(2) && cx.bases.is_occupied(3) {
            cx.score_passive(3);
        }
        score::apply_walk_bases(cx.bases, cx.batter_id);
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Resolve pending implicit runner advancement from a ball-in-play.
/// Returns player IDs of runners who scored (None for anonymous runners).
pub fn resolve_pending<S: BuildHasher, S2: BuildHasher, S3: BuildHasher>(
    half_inning: usize,
    pending: &PendingImplicit,
    bases: &mut BaseState,
    explicit_br: &HashSet<String, S>,
    handled_bases: &HashSet<usize, S3>,
    runs_by_half: &mut HashMap<usize, i32, S2>,
    half_stats: &mut Vec<RawStats>,
) -> Vec<Option<String>> {
    if pending.outs_after_play >= 3 && pending.play_result.is_advance_runners_out() {
        return Vec::new();
    }

    let mut cx = Ctx {
        hi: half_inning,
        snap: &pending.snapshot,
        bases,
        explicit: explicit_br,
        handled_bases,
        batter_id: pending.batter_id.as_deref(),
        runs: runs_by_half,
        stats: half_stats,
        scored: Vec::new(),
    };

    match pending.play_result {
        PlayResult::Triple => resolve_triple(&mut cx),
        PlayResult::Double => resolve_double(&mut cx),
        PlayResult::Single => {
            resolve_single(&mut cx, pending.play_type == Some(BipPlayType::FlyBall));
        }
        PlayResult::BatterOutAdvanceRunners
        | PlayResult::SacrificeFly
        | PlayResult::SacrificeBunt => resolve_advance_out(&mut cx),
        PlayResult::FieldersChoice => resolve_fielders_choice(&mut cx),
        PlayResult::Error => resolve_error(&mut cx),
        PlayResult::DroppedThirdStrike => resolve_dropped_third(&mut cx, pending.cause),
        _ => {}
    }

    cx.scored
}
