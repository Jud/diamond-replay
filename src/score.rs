use std::collections::HashMap;
use std::hash::BuildHasher;

use crate::state::{BaseOccupant, BaseState};

/// Record one run in the given half-inning.
pub fn score_run<S: BuildHasher>(half_inning: usize, runs_by_half: &mut HashMap<usize, i32, S>) {
    *runs_by_half.entry(half_inning).or_insert(0) += 1;
}

/// Undo one run in the given half-inning (reverses a prior `score_run`).
pub fn undo_score_run<S: BuildHasher>(
    half_inning: usize,
    runs_by_half: &mut HashMap<usize, i32, S>,
) {
    let entry = runs_by_half.entry(half_inning).or_insert(0);
    *entry -= 1;
    if *entry == 0 {
        runs_by_half.remove(&half_inning);
    }
}

/// Check if bases are loaded and score if so (walk/HBP).
/// Returns true if a run scored.
pub fn force_advance_walk_score<S: BuildHasher>(
    half_inning: usize,
    bases: &BaseState,
    runs_by_half: &mut HashMap<usize, i32, S>,
) -> bool {
    if bases.is_occupied(1) && bases.is_occupied(2) && bases.is_occupied(3) {
        score_run(half_inning, runs_by_half);
        true
    } else {
        false
    }
}

/// Perform the base-state mutations for a walk/HBP force advance.
pub fn apply_walk_bases(bases: &mut BaseState, batter_id: Option<&str>) {
    let snap1 = bases.get(1).cloned();
    let snap2 = bases.get(2).cloned();

    if snap2.is_some() && snap1.is_some() {
        bases.set(3, snap2);
    }
    if snap1.is_some() {
        bases.set(2, snap1);
    }
    let occupant = match batter_id {
        Some(id) if bases.find_by_id(id).is_none() => BaseOccupant::Player(id.to_string()),
        _ => BaseOccupant::Anonymous,
    };
    bases.set(1, Some(occupant));
}

/// Apply scorer-entered team score totals.
///
/// # Panics
///
/// Panics if `team_halves` is unexpectedly empty after the empty check
/// (should not happen in practice).
pub fn apply_score_override<S: BuildHasher>(
    half_inning: usize,
    home_id: &str,
    away_id: &str,
    runs_by_half: &mut HashMap<usize, i32, S>,
    scores: &[ScoreOverrideEntry],
) {
    for item in scores {
        let parity = if item.team_id == away_id {
            0
        } else if item.team_id == home_id {
            1
        } else {
            continue;
        };

        let team_halves: Vec<usize> = (parity..=half_inning).step_by(2).collect();
        if team_halves.is_empty() {
            continue;
        }

        let current_total: i32 = team_halves
            .iter()
            .map(|hi| runs_by_half.get(hi).copied().unwrap_or(0))
            .sum();
        let delta = item.score - current_total;

        if delta == 0 {
            continue;
        }

        if delta > 0 {
            let hi = *team_halves.last().unwrap();
            *runs_by_half.entry(hi).or_insert(0) += delta;
            continue;
        }

        let mut remaining = -delta;
        for &hi in team_halves.iter().rev() {
            let cur = runs_by_half.get(&hi).copied().unwrap_or(0);
            if cur <= 0 {
                continue;
            }
            let take = cur.min(remaining);
            let new_val = cur - take;
            if new_val > 0 {
                runs_by_half.insert(hi, new_val);
            } else {
                runs_by_half.remove(&hi);
            }
            remaining -= take;
            if remaining == 0 {
                break;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScoreOverrideEntry {
    pub team_id: String,
    pub score: i32,
}
