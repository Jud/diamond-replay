// Pure computation functions for derived baseball statistics.
//
// This module takes raw counting stats and produces derived rate stats.
// It has no state and no event handling — it is called after replay
// completes to fill in the derived fields.

// ---------------------------------------------------------------------------
// wOBA weights
// ---------------------------------------------------------------------------

/// Season-level linear weights for the wOBA calculation.
pub struct WobaWeights {
    pub bb: f64,
    pub hbp: f64,
    pub single: f64,
    pub double: f64,
    pub triple: f64,
    pub hr: f64,
}

/// Default wOBA weights (MLB 2023 approximation).
pub const DEFAULT_WOBA_WEIGHTS: WobaWeights = WobaWeights {
    bb: 0.690,
    hbp: 0.720,
    single: 0.880,
    double: 1.245,
    triple: 1.575,
    hr: 2.015,
};

// ---------------------------------------------------------------------------
// FIP constants
// ---------------------------------------------------------------------------

/// League-level constants for FIP and ERA calculations.
pub struct FipConstants {
    pub constant: f64,
    pub innings_per_game: f64,
}

/// Default FIP constants (MLB approximation).
pub const DEFAULT_FIP_CONSTANTS: FipConstants = FipConstants {
    constant: 3.10,
    innings_per_game: 9.0,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Safe integer division returning `None` when the denominator is zero.
fn safe_div(num: i32, den: i32) -> Option<f64> {
    if den == 0 {
        None
    } else {
        Some(f64::from(num) / f64::from(den))
    }
}

/// Safe float division returning `None` when the denominator is zero.
fn safe_div_f64(num: f64, den: f64) -> Option<f64> {
    if den == 0.0 {
        None
    } else {
        Some(num / den)
    }
}

/// Format outs recorded as an innings-pitched display string.
///
/// Baseball convention: the fractional part is expressed in thirds.
/// 20 outs becomes `"6.2"`, 21 outs becomes `"7.0"`, etc.
#[must_use]
pub fn format_ip(outs: i32) -> String {
    let whole = outs / 3;
    let thirds = outs % 3;
    format!("{whole}.{thirds}")
}

// ---------------------------------------------------------------------------
// Batting — raw input / derived output
// ---------------------------------------------------------------------------

/// Raw counting stats that feed the batting computation.
///
/// The caller unpacks whatever struct it has into this shape before
/// calling [`compute_batting`].
pub struct BattingRaw {
    pub pa: i32,
    pub singles: i32,
    pub doubles: i32,
    pub triples: i32,
    pub home_runs: i32,
    pub bb: i32,
    pub hbp: i32,
    pub ci: i32,
    pub k: i32,
    pub sac_fly: i32,
    pub sac_bunt: i32,
    pub sb: i32,
    pub cs: i32,
    pub ground_balls: i32,
    pub fly_balls: i32,
    pub line_drives: i32,
    pub pop_ups: i32,
    pub pitches_seen: i32,
    pub qab: i32,
    pub competitive_ab: i32,
    pub hard_hit_balls: i32,
}

/// Derived batting statistics produced by [`compute_batting`].
pub struct BattingDerived {
    pub ab: i32,
    pub hits: i32,
    pub tb: i32,
    pub xbh: i32,
    pub avg: Option<f64>,
    pub obp: Option<f64>,
    pub slg: Option<f64>,
    pub ops: Option<f64>,
    pub iso: Option<f64>,
    pub babip: Option<f64>,
    pub k_pct: Option<f64>,
    pub bb_pct: Option<f64>,
    pub bb_k: Option<f64>,
    pub woba: Option<f64>,
    pub gb_pct: Option<f64>,
    pub fb_pct: Option<f64>,
    pub ld_pct: Option<f64>,
    pub hr_fb: Option<f64>,
    pub p_pa: Option<f64>,
    pub qab_pct: Option<f64>,
    pub competitive_pct: Option<f64>,
    pub hard_hit_pct: Option<f64>,
    pub sb_pct: Option<f64>,
}

/// Compute all derived batting statistics from raw counting stats.
#[must_use]
pub fn compute_batting(raw: &BattingRaw, weights: &WobaWeights) -> BattingDerived {
    let hits = raw.singles + raw.doubles + raw.triples + raw.home_runs;
    let ab = raw.pa - raw.bb - raw.hbp - raw.sac_fly - raw.sac_bunt - raw.ci;
    let tb = raw.singles + 2 * raw.doubles + 3 * raw.triples + 4 * raw.home_runs;
    let xbh = raw.doubles + raw.triples + raw.home_runs;

    let avg = safe_div(hits, ab);
    let slg = safe_div(tb, ab);
    let obp = safe_div(hits + raw.bb + raw.hbp, ab + raw.bb + raw.hbp + raw.sac_fly);
    let ops = match (obp, slg) {
        (Some(o), Some(s)) => Some(o + s),
        _ => None,
    };
    let iso = match (slg, avg) {
        (Some(s), Some(a)) => Some(s - a),
        _ => None,
    };

    // BABIP = (H - HR) / (AB - K - HR + SF)
    let babip = safe_div(
        hits - raw.home_runs,
        ab - raw.k - raw.home_runs + raw.sac_fly,
    );

    let k_pct = safe_div(raw.k, raw.pa);
    let bb_pct = safe_div(raw.bb, raw.pa);
    let bb_k = safe_div(raw.bb, raw.k);

    // wOBA numerator (float) / denominator (int)
    let woba_num = weights.bb * f64::from(raw.bb)
        + weights.hbp * f64::from(raw.hbp)
        + weights.single * f64::from(raw.singles)
        + weights.double * f64::from(raw.doubles)
        + weights.triple * f64::from(raw.triples)
        + weights.hr * f64::from(raw.home_runs);
    let woba_den = ab + raw.bb + raw.sac_fly + raw.hbp;
    let woba = if woba_den == 0 {
        None
    } else {
        Some(woba_num / f64::from(woba_den))
    };

    // Batted-ball distribution
    let total_bip = raw.ground_balls + raw.fly_balls + raw.line_drives + raw.pop_ups;
    let gb_pct = safe_div(raw.ground_balls, total_bip);
    let fb_pct = safe_div(raw.fly_balls, total_bip);
    let ld_pct = safe_div(raw.line_drives, total_bip);
    let hr_fb = safe_div(raw.home_runs, raw.fly_balls);

    let p_pa = safe_div(raw.pitches_seen, raw.pa);
    let qab_pct = safe_div(raw.qab, raw.pa);
    let competitive_pct = safe_div(raw.competitive_ab, raw.pa);
    let hard_hit_pct = safe_div(raw.hard_hit_balls, total_bip);
    let sb_pct = safe_div(raw.sb, raw.sb + raw.cs);

    BattingDerived {
        ab,
        hits,
        tb,
        xbh,
        avg,
        obp,
        slg,
        ops,
        iso,
        babip,
        k_pct,
        bb_pct,
        bb_k,
        woba,
        gb_pct,
        fb_pct,
        ld_pct,
        hr_fb,
        p_pa,
        qab_pct,
        competitive_pct,
        hard_hit_pct,
        sb_pct,
    }
}

// ---------------------------------------------------------------------------
// Pitching — raw input / derived output
// ---------------------------------------------------------------------------

/// Raw counting stats that feed the pitching computation.
pub struct PitchingRaw {
    pub outs_recorded: i32,
    pub hits_allowed: i32,
    pub hr_allowed: i32,
    pub bb: i32,
    pub hbp: i32,
    pub k: i32,
    pub earned_runs_allowed: i32,
    pub runs_allowed: i32,
    pub bf: i32,
    pub pitches: i32,
    pub strikes_swinging: i32,
    pub strikes_looking: i32,
    pub first_pitch_strikes: i32,
    pub fouls: i32,
    pub ground_balls: i32,
    pub fly_balls: i32,
    pub line_drives: i32,
    pub pop_ups: i32,
    pub bip: i32,
}

/// Derived pitching statistics produced by [`compute_pitching`].
pub struct PitchingDerived {
    pub ip: f64,
    pub ip_display: String,
    pub era: Option<f64>,
    pub whip: Option<f64>,
    pub k9: Option<f64>,
    pub bb9: Option<f64>,
    pub h9: Option<f64>,
    pub hr9: Option<f64>,
    pub k_bb: Option<f64>,
    pub fip: Option<f64>,
    pub k_pct: Option<f64>,
    pub bb_pct: Option<f64>,
    pub k_bb_pct: Option<f64>,
    pub babip: Option<f64>,
    pub hr_fb: Option<f64>,
    pub gb_pct: Option<f64>,
    pub fb_pct: Option<f64>,
    pub ld_pct: Option<f64>,
    pub sw_str_pct: Option<f64>,
    pub csw_pct: Option<f64>,
    pub c_str_pct: Option<f64>,
    pub fps_pct: Option<f64>,
    pub foul_pct: Option<f64>,
    pub game_score: i32,
    pub pitches_per_ip: Option<f64>,
}

/// Compute all derived pitching statistics from raw counting stats.
#[must_use]
pub fn compute_pitching(raw: &PitchingRaw, fip_constants: &FipConstants) -> PitchingDerived {
    let ip = f64::from(raw.outs_recorded) / 3.0;
    let ip_display = format_ip(raw.outs_recorded);

    let era = safe_div_f64(
        f64::from(raw.earned_runs_allowed) * fip_constants.innings_per_game,
        ip,
    );
    let whip = safe_div_f64(f64::from(raw.bb + raw.hits_allowed), ip);
    let k9 = safe_div_f64(f64::from(raw.k) * 9.0, ip);
    let bb9 = safe_div_f64(f64::from(raw.bb) * 9.0, ip);
    let h9 = safe_div_f64(f64::from(raw.hits_allowed) * 9.0, ip);
    let hr9 = safe_div_f64(f64::from(raw.hr_allowed) * 9.0, ip);
    let k_bb = safe_div(raw.k, raw.bb);

    // FIP = ((13*HR + 3*(BB+HBP) - 2*K) / IP) + constant
    let fip_num = f64::from(13 * raw.hr_allowed + 3 * (raw.bb + raw.hbp) - 2 * raw.k);
    let fip = safe_div_f64(fip_num, ip).map(|v| v + fip_constants.constant);

    let k_pct = safe_div(raw.k, raw.bf);
    let bb_pct = safe_div(raw.bb, raw.bf);
    let k_bb_pct = match (k_pct, bb_pct) {
        (Some(k), Some(b)) => Some(k - b),
        _ => None,
    };

    // Pitching BABIP = (H - HR) / (BIP - HR)
    let babip = safe_div(raw.hits_allowed - raw.hr_allowed, raw.bip - raw.hr_allowed);

    let hr_fb = safe_div(raw.hr_allowed, raw.fly_balls);

    let total_bip = raw.ground_balls + raw.fly_balls + raw.line_drives + raw.pop_ups;
    let gb_pct = safe_div(raw.ground_balls, total_bip);
    let fb_pct = safe_div(raw.fly_balls, total_bip);
    let ld_pct = safe_div(raw.line_drives, total_bip);

    let sw_str_pct = safe_div(raw.strikes_swinging, raw.pitches);
    let csw_pct = safe_div(raw.strikes_looking + raw.strikes_swinging, raw.pitches);
    let c_str_pct = safe_div(raw.strikes_looking, raw.pitches);
    let fps_pct = safe_div(raw.first_pitch_strikes, raw.bf);
    let foul_pct = safe_div(raw.fouls, raw.pitches);

    // Game Score (Bill James):
    //   50 + outs + 2*(outs - 12).max(0)/3 + K - 2*H - 4*ER - 2*(R-ER) - BB
    //
    // The "+2 per IP after the 4th inning" means +2 for every full inning
    // beyond the 4th. An inning is 3 outs, so outs in inning 5+ =
    // (outs - 12).max(0). Each group of 3 such outs is one inning worth +2.
    // We use integer division: extra_inning_bonus = 2 * (extra_outs / 3).
    let extra_outs = (raw.outs_recorded - 12).max(0);
    let game_score = 50 + raw.outs_recorded + 2 * (extra_outs / 3) + raw.k
        - 2 * raw.hits_allowed
        - 4 * raw.earned_runs_allowed
        - 2 * (raw.runs_allowed - raw.earned_runs_allowed)
        - raw.bb;

    let pitches_per_ip = safe_div_f64(f64::from(raw.pitches), ip);

    PitchingDerived {
        ip,
        ip_display,
        era,
        whip,
        k9,
        bb9,
        h9,
        hr9,
        k_bb,
        fip,
        k_pct,
        bb_pct,
        k_bb_pct,
        babip,
        hr_fb,
        gb_pct,
        fb_pct,
        ld_pct,
        sw_str_pct,
        csw_pct,
        c_str_pct,
        fps_pct,
        foul_pct,
        game_score,
        pitches_per_ip,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Tolerance for floating-point comparisons.
    const EPS: f64 = 1e-6;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    fn assert_approx(label: &str, got: Option<f64>, expected: f64) {
        let v = got.unwrap_or_else(|| panic!("{label}: expected Some, got None"));
        assert!(
            approx_eq(v, expected),
            "{label}: expected {expected}, got {v}"
        );
    }

    fn assert_none(label: &str, got: Option<f64>) {
        assert!(got.is_none(), "{label}: expected None, got {got:?}");
    }

    // -- format_ip edge cases -----------------------------------------------

    #[test]
    fn test_format_ip_zero() {
        assert_eq!(format_ip(0), "0.0");
    }

    #[test]
    fn test_format_ip_one() {
        assert_eq!(format_ip(1), "0.1");
    }

    #[test]
    fn test_format_ip_two() {
        assert_eq!(format_ip(2), "0.2");
    }

    #[test]
    fn test_format_ip_three() {
        assert_eq!(format_ip(3), "1.0");
    }

    #[test]
    fn test_format_ip_nineteen() {
        assert_eq!(format_ip(19), "6.1");
    }

    #[test]
    fn test_format_ip_twenty() {
        assert_eq!(format_ip(20), "6.2");
    }

    #[test]
    fn test_format_ip_twenty_one() {
        assert_eq!(format_ip(21), "7.0");
    }

    // -- Batting: normal realistic case ------------------------------------

    fn realistic_batting_raw() -> BattingRaw {
        // A decent hitter: 4-for-10, 1 2B, 1 HR, 2 BB, 1 K, 1 SF
        BattingRaw {
            pa: 15,
            singles: 2,
            doubles: 1,
            triples: 0,
            home_runs: 1,
            bb: 2,
            hbp: 1,
            ci: 0,
            k: 3,
            sac_fly: 1,
            sac_bunt: 0,
            sb: 2,
            cs: 1,
            ground_balls: 3,
            fly_balls: 4,
            line_drives: 2,
            pop_ups: 1,
            pitches_seen: 60,
            qab: 8,
            competitive_ab: 6,
            hard_hit_balls: 3,
        }
    }

    #[test]
    fn test_batting_normal_counting() {
        let raw = realistic_batting_raw();
        let d = compute_batting(&raw, &DEFAULT_WOBA_WEIGHTS);

        // ab = 15 - 2 - 1 - 1 - 0 = 11
        assert_eq!(d.ab, 11);
        // hits = 2 + 1 + 0 + 1 = 4
        assert_eq!(d.hits, 4);
        // tb = 2 + 2 + 0 + 4 = 8
        assert_eq!(d.tb, 8);
        // xbh = 1 + 0 + 1 = 2
        assert_eq!(d.xbh, 2);
    }

    #[test]
    fn test_batting_normal_rates() {
        let raw = realistic_batting_raw();
        let d = compute_batting(&raw, &DEFAULT_WOBA_WEIGHTS);

        // avg = 4/11
        assert_approx("avg", d.avg, 4.0 / 11.0);
        // slg = 8/11
        assert_approx("slg", d.slg, 8.0 / 11.0);
        // obp = (4+2+1) / (11+2+1+1) = 7/15
        assert_approx("obp", d.obp, 7.0 / 15.0);
        // ops = obp + slg
        assert_approx("ops", d.ops, 7.0 / 15.0 + 8.0 / 11.0);
        // iso = slg - avg
        assert_approx("iso", d.iso, 8.0 / 11.0 - 4.0 / 11.0);
    }

    #[test]
    fn test_batting_normal_babip() {
        let raw = realistic_batting_raw();
        let d = compute_batting(&raw, &DEFAULT_WOBA_WEIGHTS);

        // babip = (4-1) / (11-3-1+1) = 3/8
        assert_approx("babip", d.babip, 3.0 / 8.0);
    }

    #[test]
    fn test_batting_normal_pct() {
        let raw = realistic_batting_raw();
        let d = compute_batting(&raw, &DEFAULT_WOBA_WEIGHTS);

        assert_approx("k_pct", d.k_pct, 3.0 / 15.0);
        assert_approx("bb_pct", d.bb_pct, 2.0 / 15.0);
        assert_approx("bb_k", d.bb_k, 2.0 / 3.0);
    }

    #[test]
    fn test_batting_normal_woba() {
        let raw = realistic_batting_raw();
        let w = &DEFAULT_WOBA_WEIGHTS;
        let d = compute_batting(&raw, w);

        let num = w.bb * 2.0
            + w.hbp * 1.0
            + w.single * 2.0
            + w.double * 1.0
            + w.triple * 0.0
            + w.hr * 1.0;
        // den = 11 + 2 + 1 + 1 = 15
        let expected = num / 15.0;
        assert_approx("woba", d.woba, expected);
    }

    #[test]
    fn test_batting_normal_batted_ball() {
        let raw = realistic_batting_raw();
        let d = compute_batting(&raw, &DEFAULT_WOBA_WEIGHTS);

        // total_bip = 3+4+2+1 = 10
        assert_approx("gb_pct", d.gb_pct, 3.0 / 10.0);
        assert_approx("fb_pct", d.fb_pct, 4.0 / 10.0);
        assert_approx("ld_pct", d.ld_pct, 2.0 / 10.0);
        assert_approx("hr_fb", d.hr_fb, 1.0 / 4.0);
    }

    #[test]
    fn test_batting_normal_misc() {
        let raw = realistic_batting_raw();
        let d = compute_batting(&raw, &DEFAULT_WOBA_WEIGHTS);

        assert_approx("p_pa", d.p_pa, 60.0 / 15.0);
        assert_approx("qab_pct", d.qab_pct, 8.0 / 15.0);
        assert_approx("competitive_pct", d.competitive_pct, 6.0 / 15.0);
        assert_approx("hard_hit_pct", d.hard_hit_pct, 3.0 / 10.0);
        assert_approx("sb_pct", d.sb_pct, 2.0 / 3.0);
    }

    // -- Batting: zero PA ---------------------------------------------------

    #[test]
    fn test_batting_zero_pa() {
        let raw = BattingRaw {
            pa: 0,
            singles: 0,
            doubles: 0,
            triples: 0,
            home_runs: 0,
            bb: 0,
            hbp: 0,
            ci: 0,
            k: 0,
            sac_fly: 0,
            sac_bunt: 0,
            sb: 0,
            cs: 0,
            ground_balls: 0,
            fly_balls: 0,
            line_drives: 0,
            pop_ups: 0,
            pitches_seen: 0,
            qab: 0,
            competitive_ab: 0,
            hard_hit_balls: 0,
        };
        let d = compute_batting(&raw, &DEFAULT_WOBA_WEIGHTS);

        assert_eq!(d.ab, 0);
        assert_eq!(d.hits, 0);
        assert_eq!(d.tb, 0);
        assert_eq!(d.xbh, 0);

        assert_none("avg", d.avg);
        assert_none("obp", d.obp);
        assert_none("slg", d.slg);
        assert_none("ops", d.ops);
        assert_none("iso", d.iso);
        assert_none("babip", d.babip);
        assert_none("k_pct", d.k_pct);
        assert_none("bb_pct", d.bb_pct);
        assert_none("bb_k", d.bb_k);
        assert_none("woba", d.woba);
        assert_none("gb_pct", d.gb_pct);
        assert_none("fb_pct", d.fb_pct);
        assert_none("ld_pct", d.ld_pct);
        assert_none("hr_fb", d.hr_fb);
        assert_none("p_pa", d.p_pa);
        assert_none("qab_pct", d.qab_pct);
        assert_none("competitive_pct", d.competitive_pct);
        assert_none("hard_hit_pct", d.hard_hit_pct);
        assert_none("sb_pct", d.sb_pct);
    }

    // -- Batting: all strikeouts (0 hits, 0 BB) -----------------------------

    #[test]
    fn test_batting_all_strikeouts() {
        let raw = BattingRaw {
            pa: 4,
            singles: 0,
            doubles: 0,
            triples: 0,
            home_runs: 0,
            bb: 0,
            hbp: 0,
            ci: 0,
            k: 4,
            sac_fly: 0,
            sac_bunt: 0,
            sb: 0,
            cs: 0,
            ground_balls: 0,
            fly_balls: 0,
            line_drives: 0,
            pop_ups: 0,
            pitches_seen: 16,
            qab: 0,
            competitive_ab: 4,
            hard_hit_balls: 0,
        };
        let d = compute_batting(&raw, &DEFAULT_WOBA_WEIGHTS);

        assert_eq!(d.ab, 4);
        assert_eq!(d.hits, 0);
        assert_approx("avg", d.avg, 0.0);
        assert_approx("obp", d.obp, 0.0);
        assert_approx("slg", d.slg, 0.0);
        assert_approx("ops", d.ops, 0.0);
        assert_approx("iso", d.iso, 0.0);
        assert_approx("k_pct", d.k_pct, 1.0);
        assert_approx("bb_pct", d.bb_pct, 0.0);
        assert_approx("bb_k", d.bb_k, 0.0);
        assert_approx("woba", d.woba, 0.0);

        // BABIP denominator = 4 - 4 - 0 + 0 = 0 → None
        assert_none("babip", d.babip);
        // No BIP → None
        assert_none("gb_pct", d.gb_pct);
        assert_none("sb_pct", d.sb_pct);
    }

    // -- Pitching: normal realistic case ------------------------------------

    fn realistic_pitching_raw() -> PitchingRaw {
        // A quality start: 7 IP, 5 H, 1 HR, 2 BB, 8 K, 2 ER
        PitchingRaw {
            outs_recorded: 21,
            hits_allowed: 5,
            hr_allowed: 1,
            bb: 2,
            hbp: 0,
            k: 8,
            earned_runs_allowed: 2,
            runs_allowed: 2,
            bf: 28,
            pitches: 100,
            strikes_swinging: 15,
            strikes_looking: 12,
            first_pitch_strikes: 18,
            fouls: 10,
            ground_balls: 5,
            fly_balls: 4,
            line_drives: 3,
            pop_ups: 2,
            bip: 14,
        }
    }

    #[test]
    fn test_pitching_normal_ip() {
        let raw = realistic_pitching_raw();
        let d = compute_pitching(&raw, &DEFAULT_FIP_CONSTANTS);

        assert!(approx_eq(d.ip, 7.0));
        assert_eq!(d.ip_display, "7.0");
    }

    #[test]
    fn test_pitching_normal_rates() {
        let raw = realistic_pitching_raw();
        let d = compute_pitching(&raw, &DEFAULT_FIP_CONSTANTS);

        // era = (2/7) * 9 = 18/7
        assert_approx("era", d.era, 18.0 / 7.0);
        // whip = (2+5) / 7 = 1.0
        assert_approx("whip", d.whip, 1.0);
        // k9 = 8/7 * 9
        assert_approx("k9", d.k9, 72.0 / 7.0);
        // bb9 = 2/7 * 9
        assert_approx("bb9", d.bb9, 18.0 / 7.0);
        // h9 = 5/7 * 9
        assert_approx("h9", d.h9, 45.0 / 7.0);
        // hr9 = 1/7 * 9
        assert_approx("hr9", d.hr9, 9.0 / 7.0);
        // k_bb = 8/2 = 4
        assert_approx("k_bb", d.k_bb, 4.0);
    }

    #[test]
    fn test_pitching_normal_fip() {
        let raw = realistic_pitching_raw();
        let d = compute_pitching(&raw, &DEFAULT_FIP_CONSTANTS);

        // FIP = (13*1 + 3*(2+0) - 2*8) / 7 + 3.10
        //     = (13 + 6 - 16) / 7 + 3.10
        //     = 3/7 + 3.10
        let expected = 3.0 / 7.0 + 3.10;
        assert_approx("fip", d.fip, expected);
    }

    #[test]
    fn test_pitching_normal_pct() {
        let raw = realistic_pitching_raw();
        let d = compute_pitching(&raw, &DEFAULT_FIP_CONSTANTS);

        assert_approx("k_pct", d.k_pct, 8.0 / 28.0);
        assert_approx("bb_pct", d.bb_pct, 2.0 / 28.0);
        assert_approx("k_bb_pct", d.k_bb_pct, 6.0 / 28.0);
    }

    #[test]
    fn test_pitching_normal_babip() {
        let raw = realistic_pitching_raw();
        let d = compute_pitching(&raw, &DEFAULT_FIP_CONSTANTS);

        // babip = (5-1) / (14-1) = 4/13
        assert_approx("babip", d.babip, 4.0 / 13.0);
    }

    #[test]
    fn test_pitching_normal_batted_ball() {
        let raw = realistic_pitching_raw();
        let d = compute_pitching(&raw, &DEFAULT_FIP_CONSTANTS);

        // total_bip = 5+4+3+2 = 14
        assert_approx("gb_pct", d.gb_pct, 5.0 / 14.0);
        assert_approx("fb_pct", d.fb_pct, 4.0 / 14.0);
        assert_approx("ld_pct", d.ld_pct, 3.0 / 14.0);
        assert_approx("hr_fb", d.hr_fb, 1.0 / 4.0);
    }

    #[test]
    fn test_pitching_normal_pitch_pct() {
        let raw = realistic_pitching_raw();
        let d = compute_pitching(&raw, &DEFAULT_FIP_CONSTANTS);

        assert_approx("sw_str_pct", d.sw_str_pct, 15.0 / 100.0);
        assert_approx("csw_pct", d.csw_pct, 27.0 / 100.0);
        assert_approx("c_str_pct", d.c_str_pct, 12.0 / 100.0);
        assert_approx("fps_pct", d.fps_pct, 18.0 / 28.0);
        assert_approx("foul_pct", d.foul_pct, 10.0 / 100.0);
    }

    #[test]
    fn test_pitching_normal_game_score() {
        let raw = realistic_pitching_raw();
        let d = compute_pitching(&raw, &DEFAULT_FIP_CONSTANTS);

        // game_score = 50 + 21 + 2*((21-12).max(0)/3) + 8 - 2*5 - 4*2 - 2*(2-2) - 2
        //            = 50 + 21 + 2*(9/3) + 8 - 10 - 8 - 0 - 2
        //            = 50 + 21 + 6 + 8 - 10 - 8 - 2
        //            = 65
        assert_eq!(d.game_score, 65);
    }

    #[test]
    fn test_pitching_normal_pitches_per_ip() {
        let raw = realistic_pitching_raw();
        let d = compute_pitching(&raw, &DEFAULT_FIP_CONSTANTS);

        assert_approx("pitches_per_ip", d.pitches_per_ip, 100.0 / 7.0);
    }

    // -- Pitching: zero IP --------------------------------------------------

    #[test]
    fn test_pitching_zero_ip() {
        let raw = PitchingRaw {
            outs_recorded: 0,
            hits_allowed: 0,
            hr_allowed: 0,
            bb: 0,
            hbp: 0,
            k: 0,
            earned_runs_allowed: 0,
            runs_allowed: 0,
            bf: 0,
            pitches: 0,
            strikes_swinging: 0,
            strikes_looking: 0,
            first_pitch_strikes: 0,
            fouls: 0,
            ground_balls: 0,
            fly_balls: 0,
            line_drives: 0,
            pop_ups: 0,
            bip: 0,
        };
        let d = compute_pitching(&raw, &DEFAULT_FIP_CONSTANTS);

        assert!(approx_eq(d.ip, 0.0));
        assert_eq!(d.ip_display, "0.0");

        assert_none("era", d.era);
        assert_none("whip", d.whip);
        assert_none("k9", d.k9);
        assert_none("bb9", d.bb9);
        assert_none("h9", d.h9);
        assert_none("hr9", d.hr9);
        assert_none("k_bb", d.k_bb);
        assert_none("fip", d.fip);
        assert_none("k_pct", d.k_pct);
        assert_none("bb_pct", d.bb_pct);
        assert_none("k_bb_pct", d.k_bb_pct);
        assert_none("babip", d.babip);
        assert_none("hr_fb", d.hr_fb);
        assert_none("gb_pct", d.gb_pct);
        assert_none("fb_pct", d.fb_pct);
        assert_none("ld_pct", d.ld_pct);
        assert_none("sw_str_pct", d.sw_str_pct);
        assert_none("csw_pct", d.csw_pct);
        assert_none("c_str_pct", d.c_str_pct);
        assert_none("fps_pct", d.fps_pct);
        assert_none("foul_pct", d.foul_pct);
        assert_none("pitches_per_ip", d.pitches_per_ip);

        // Game score with no outs: 50 + 0 + 0 + 0 - 0 - 0 - 0 - 0 = 50
        assert_eq!(d.game_score, 50);
    }

    // -- Pitching: perfect game ---------------------------------------------

    #[test]
    fn test_pitching_perfect_game() {
        // 27 outs, 0 hits, 0 BB, 0 runs, 12 K
        let raw = PitchingRaw {
            outs_recorded: 27,
            hits_allowed: 0,
            hr_allowed: 0,
            bb: 0,
            hbp: 0,
            k: 12,
            earned_runs_allowed: 0,
            runs_allowed: 0,
            bf: 27,
            pitches: 90,
            strikes_swinging: 20,
            strikes_looking: 15,
            first_pitch_strikes: 20,
            fouls: 8,
            ground_balls: 7,
            fly_balls: 5,
            line_drives: 2,
            pop_ups: 1,
            bip: 15,
        };
        let d = compute_pitching(&raw, &DEFAULT_FIP_CONSTANTS);

        assert!(approx_eq(d.ip, 9.0));
        assert_eq!(d.ip_display, "9.0");
        assert_approx("era", d.era, 0.0);
        assert_approx("whip", d.whip, 0.0);

        // FIP = (13*0 + 3*0 - 2*12) / 9 + 3.10 = -24/9 + 3.10
        assert_approx("fip", d.fip, -24.0 / 9.0 + 3.10);

        // game_score = 50 + 27 + 2*((27-12)/3) + 12 - 0 - 0 - 0 - 0
        //            = 50 + 27 + 2*5 + 12 = 99
        assert_eq!(d.game_score, 99);

        // k_bb: denominator is 0 → None
        assert_none("k_bb", d.k_bb);
    }

    // -- safe_div edge cases ------------------------------------------------

    #[test]
    fn test_safe_div_normal() {
        assert!(approx_eq(safe_div(3, 4).unwrap(), 0.75));
    }

    #[test]
    fn test_safe_div_zero_denominator() {
        assert!(safe_div(5, 0).is_none());
    }

    #[test]
    fn test_safe_div_zero_numerator() {
        assert!(approx_eq(safe_div(0, 4).unwrap(), 0.0));
    }

    #[test]
    fn test_safe_div_f64_zero_denominator() {
        assert!(safe_div_f64(5.0, 0.0).is_none());
    }
}
