use diamond_replay::{replay_from_json, replay_from_json_no_steal_home};
use std::collections::HashMap;

fn load_box_scores() -> HashMap<String, (Vec<i32>, Vec<i32>)> {
    let json = include_str!("../testdata/box_scores.json");
    let data: serde_json::Value = serde_json::from_str(json).unwrap();
    let mut map = HashMap::new();
    for (key, val) in data.as_object().unwrap() {
        let away: Vec<i32> = val["away"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect();
        let home: Vec<i32> = val["home"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap() as i32)
            .collect();
        map.insert(key.clone(), (away, home));
    }
    map
}

macro_rules! game_test {
    ($name:ident, $file:literal, $game_key:literal) => {
        #[test]
        fn $name() {
            let json = include_str!(concat!("../testdata/", $file));
            let result = replay_from_json(json).expect("replay should succeed");
            let box_scores = load_box_scores();
            let (expected_away, expected_home) = &box_scores[$game_key];

            assert_eq!(
                &result.linescore_away, expected_away,
                "{} away linescore mismatch: got {:?}, expected {:?}",
                $game_key, result.linescore_away, expected_away
            );
            assert_eq!(
                &result.linescore_home, expected_home,
                "{} home linescore mismatch: got {:?}, expected {:?}",
                $game_key, result.linescore_home, expected_home
            );

            // Verify player runs sum == linescore total for both teams
            let away_total: i32 = result.linescore_away.iter().sum();
            let home_total: i32 = result.linescore_home.iter().sum();
            let away_player_runs: i32 = result
                .player_stats
                .values()
                .filter(|p| p.team_id == result.away_id)
                .map(|p| p.batting.runs)
                .sum();
            let home_player_runs: i32 = result
                .player_stats
                .values()
                .filter(|p| p.team_id == result.home_id)
                .map(|p| p.batting.runs)
                .sum();
            assert_eq!(
                away_player_runs, away_total,
                "{} away player runs mismatch: player_sum={}, linescore={}",
                $game_key, away_player_runs, away_total
            );
            assert_eq!(
                home_player_runs, home_total,
                "{} home player runs mismatch: player_sum={}, linescore={}",
                $game_key, home_player_runs, home_total
            );

            // Invariant: AB + BB + HBP + SF + SAC == PA for each player with PA > 0
            for ps in result.player_stats.values() {
                if ps.batting.pa > 0 {
                    assert_eq!(
                        ps.batting.ab + ps.batting.bb + ps.batting.hbp
                            + ps.batting.ci + ps.batting.sac_fly + ps.batting.sac_bunt,
                        ps.batting.pa,
                        "{} player {} PA invariant failed: ab({}) + bb({}) + hbp({}) + ci({}) + sf({}) + sac({}) != pa({})",
                        $game_key, ps.player_id,
                        ps.batting.ab, ps.batting.bb, ps.batting.hbp,
                        ps.batting.ci,
                        ps.batting.sac_fly, ps.batting.sac_bunt, ps.batting.pa
                    );
                }
            }

            // Invariant: hits == singles + doubles + triples + home_runs
            for ps in result.player_stats.values() {
                assert_eq!(
                    ps.batting.hits,
                    ps.batting.singles + ps.batting.doubles
                        + ps.batting.triples + ps.batting.home_runs,
                    "{} player {} hits invariant failed",
                    $game_key, ps.player_id
                );
            }
        }
    };
}

game_test!(
    test_10u_mariners_cardinals,
    "10U_Mariners_Cardinals.json",
    "10U_Mariners_Cardinals"
);
game_test!(
    test_10u_mets_brewers,
    "10U_Mets_Brewers.json",
    "10U_Mets_Brewers"
);
game_test!(
    test_10u_braves_yankees,
    "10U_Braves_Yankees.json",
    "10U_Braves_Yankees"
);
game_test!(
    test_10u_tigers_dodgers,
    "10U_Tigers_Dodgers.json",
    "10U_Tigers_Dodgers"
);
game_test!(
    test_13u_braves_padres,
    "13U_Braves_Padres.json",
    "13U_Braves_Padres"
);
game_test!(
    test_13u_mariners_brewers,
    "13U_Mariners_Brewers.json",
    "13U_Mariners_Brewers"
);
game_test!(
    test_13u_phillies_cardinals,
    "13U_Phillies_Cardinals.json",
    "13U_Phillies_Cardinals"
);
game_test!(
    test_mccabe_tigers_reds,
    "McCabe_Tigers_Reds.json",
    "McCabe_Tigers_Reds"
);
game_test!(
    test_mccabe_tigers_angels,
    "McCabe_Tigers_Angels.json",
    "McCabe_Tigers_Angels"
);
game_test!(
    test_mccabe_tigers_yankees,
    "McCabe_Tigers_Yankees.json",
    "McCabe_Tigers_Yankees"
);
game_test!(
    test_mccabe_tigers_mets,
    "McCabe_Tigers_Mets.json",
    "McCabe_Tigers_Mets"
);
game_test!(
    test_stars_vs_tigers_mar31,
    "stars_vs_tigers_mar31.json",
    "stars_vs_tigers_mar31"
);
game_test!(
    test_mariners_vs_tigers_apr1,
    "mariners_vs_tigers_apr1.json",
    "mariners_vs_tigers_apr1"
);

game_test!(
    test_10u_mariners_brewers_apr12,
    "10U_Mariners_Brewers_Apr12.json",
    "10U_Mariners_Brewers_Apr12"
);

// Linescore source: /public/game-stream-processing/organizations/{event_id}/linescore
game_test!(
    test_10u_braves_cardinals_apr25,
    "10U_Braves_Cardinals_Apr25.json",
    "10U_Braves_Cardinals_Apr25"
);
game_test!(
    test_13u_cardinals_braves_apr25,
    "13U_Cardinals_Braves_Apr25.json",
    "13U_Cardinals_Braves_Apr25"
);

/// Regression: a pitch event with a createdAt > 60 min after the previous pitch
/// is treated as a post-game scorebook edit, not a real pitch — it must not
/// extend duration_min. This Cubs game has one such edit at 4:40 PM (game ended
/// at 1:25 PM) which previously made duration report ~278 min instead of ~82.
#[test]
fn test_late_edit_pitch_does_not_extend_duration() {
    let json = include_str!("../testdata/McCabe_Tigers_Cubs_Apr18.json");
    let result = replay_from_json(json).expect("replay should succeed");
    let first = result.first_pitch_timestamp.expect("first pitch ts");
    let last = result.last_pitch_timestamp.expect("last pitch ts");
    let duration_min = (last - first) as f64 / 1000.0 / 60.0;
    assert!(
        duration_min < 150.0,
        "duration {duration_min:.1} min suggests a late-edit pitch leaked into the window"
    );
}

/// Regression: auto-scored runners must not be double-counted when the
/// confirming base_running event arrives in a later transaction.
#[test]
fn test_auto_score_no_double_count() {
    let json = include_str!("../testdata/10U_Mariners_Brewers_Apr12.json");
    let result = replay_from_json(json).expect("replay should succeed");

    let away_linescore: i32 = result.linescore_away.iter().sum();
    let home_linescore: i32 = result.linescore_home.iter().sum();
    let away_player_runs: i32 = result
        .player_stats
        .values()
        .filter(|p| p.team_id == result.away_id)
        .map(|p| p.batting.runs)
        .sum();
    let home_player_runs: i32 = result
        .player_stats
        .values()
        .filter(|p| p.team_id == result.home_id)
        .map(|p| p.batting.runs)
        .sum();

    assert_eq!(
        away_player_runs, away_linescore,
        "away player runs ({}) != linescore ({})",
        away_player_runs, away_linescore
    );
    assert_eq!(
        home_player_runs, home_linescore,
        "home player runs ({}) != linescore ({})",
        home_player_runs, home_linescore
    );
}

#[test]
fn test_substituted_error_runner_stays_unearned() {
    fn raw_event(seq: i64, event_data: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "id": format!("evt-{seq}"),
            "stream_id": "test",
            "sequence_number": seq,
            "event_data": event_data.to_string(),
        })
    }

    let events = vec![
        raw_event(
            1,
            serde_json::json!({
                "code": "set_teams",
                "attributes": {"awayId": "away", "homeId": "home"}
            }),
        ),
        raw_event(
            2,
            serde_json::json!({
                "code": "fill_lineup_index",
                "attributes": {"teamId": "away", "playerId": "p1", "index": 0}
            }),
        ),
        raw_event(
            3,
            serde_json::json!({
                "code": "fill_lineup_index",
                "attributes": {"teamId": "away", "playerId": "p2", "index": 1}
            }),
        ),
        raw_event(
            4,
            serde_json::json!({
                "code": "fill_position",
                "attributes": {"teamId": "home", "playerId": "hp", "position": "P"}
            }),
        ),
        raw_event(
            5,
            serde_json::json!({
                "code": "transaction",
                "events": [
                    {"code": "pitch", "attributes": {"result": "ball_in_play", "advancesCount": true}},
                    {"code": "ball_in_play", "attributes": {"playResult": "error", "playType": "ground_ball"}}
                ]
            }),
        ),
        raw_event(
            6,
            serde_json::json!({
                "code": "sub_players",
                "attributes": {
                    "teamId": "away",
                    "outgoingPlayerId": "p1",
                    "incomingPlayerId": "pr",
                    "applyToBaserunners": true
                }
            }),
        ),
        raw_event(
            7,
            serde_json::json!({
                "code": "transaction",
                "events": [
                    {"code": "pitch", "attributes": {"result": "ball_in_play", "advancesCount": true}},
                    {"code": "ball_in_play", "attributes": {"playResult": "home_run", "playType": "fly_ball"}}
                ]
            }),
        ),
    ];
    let json = serde_json::to_string(&events).unwrap();

    let result = replay_from_json(&json).expect("replay should succeed");

    assert_eq!(result.linescore_away.iter().sum::<i32>(), 2);
    assert_eq!(result.home_pitching.runs_allowed, 2);
    assert_eq!(result.home_pitching.earned_runs_allowed, 1);
    assert_eq!(result.player_stats["pr"].batting.runs, 1);
}

#[test]
fn test_player_stats_populated() {
    let json = include_str!("../testdata/13U_Braves_Padres.json");
    let result = replay_from_json(json).expect("replay should succeed");

    // Should have players from both teams
    assert!(
        !result.player_stats.is_empty(),
        "player_stats should not be empty"
    );

    // Sum of all player PAs should equal team PA totals
    let away_player_pa: i32 = result
        .player_stats
        .values()
        .filter(|p| p.team_id == result.away_id)
        .map(|p| p.batting.pa)
        .sum();
    let home_player_pa: i32 = result
        .player_stats
        .values()
        .filter(|p| p.team_id == result.home_id)
        .map(|p| p.batting.pa)
        .sum();

    assert_eq!(
        away_player_pa, result.away_batting.pa,
        "Away player PA sum should match team total"
    );
    assert_eq!(
        home_player_pa, result.home_batting.pa,
        "Home player PA sum should match team total"
    );

    // Player runs should sum to linescore
    let away_runs: i32 = result
        .player_stats
        .values()
        .filter(|p| p.team_id == result.away_id)
        .map(|p| p.batting.runs)
        .sum();
    let home_runs: i32 = result
        .player_stats
        .values()
        .filter(|p| p.team_id == result.home_id)
        .map(|p| p.batting.runs)
        .sum();
    let away_ls: i32 = result.linescore_away.iter().sum();
    let home_ls: i32 = result.linescore_home.iter().sum();
    assert_eq!(
        away_runs, away_ls,
        "Away player runs should match linescore total"
    );
    assert_eq!(
        home_runs, home_ls,
        "Home player runs should match linescore total"
    );

    // AB + BB + HBP + SF + SAC == PA
    for ps in result.player_stats.values() {
        if ps.batting.pa > 0 {
            assert_eq!(
                ps.batting.ab
                    + ps.batting.bb
                    + ps.batting.hbp
                    + ps.batting.ci
                    + ps.batting.sac_fly
                    + ps.batting.sac_bunt,
                ps.batting.pa,
                "Player {} PA invariant failed",
                ps.player_id
            );
        }
    }

    // hits == singles + doubles + triples + home_runs
    for ps in result.player_stats.values() {
        assert_eq!(
            ps.batting.hits,
            ps.batting.singles + ps.batting.doubles + ps.batting.triples + ps.batting.home_runs,
            "Player {} hits invariant failed",
            ps.player_id
        );
    }
}

// ---------------------------------------------------------------------------
// Little League balance invariant: runs_on_bip + runs_passive == runs_total
// ---------------------------------------------------------------------------

macro_rules! ll_balance_test {
    ($name:ident, $file:literal) => {
        #[test]
        fn $name() {
            let json = include_str!(concat!("../testdata/", $file));
            let result = replay_from_json(json).expect("replay should succeed");

            let away_total: i32 = result.linescore_away.iter().sum();
            let home_total: i32 = result.linescore_home.iter().sum();
            let away_ll = &result.away_little_league;
            let home_ll = &result.home_little_league;

            assert_eq!(
                away_ll.runs_on_bip + away_ll.runs_passive,
                away_total,
                "{} away LL balance: bip({}) + passive({}) = {} != linescore({})",
                $file,
                away_ll.runs_on_bip,
                away_ll.runs_passive,
                away_ll.runs_on_bip + away_ll.runs_passive,
                away_total
            );
            assert_eq!(
                home_ll.runs_on_bip + home_ll.runs_passive,
                home_total,
                "{} home LL balance: bip({}) + passive({}) = {} != linescore({})",
                $file,
                home_ll.runs_on_bip,
                home_ll.runs_passive,
                home_ll.runs_on_bip + home_ll.runs_passive,
                home_total
            );
        }
    };
}

ll_balance_test!(
    test_ll_balance_mariners_cardinals,
    "10U_Mariners_Cardinals.json"
);
ll_balance_test!(test_ll_balance_mets_brewers, "10U_Mets_Brewers.json");
ll_balance_test!(test_ll_balance_braves_yankees, "10U_Braves_Yankees.json");
ll_balance_test!(test_ll_balance_tigers_dodgers, "10U_Tigers_Dodgers.json");
ll_balance_test!(test_ll_balance_13u_braves_padres, "13U_Braves_Padres.json");
ll_balance_test!(
    test_ll_balance_13u_mariners_brewers,
    "13U_Mariners_Brewers.json"
);
ll_balance_test!(
    test_ll_balance_13u_phillies_cardinals,
    "13U_Phillies_Cardinals.json"
);
ll_balance_test!(test_ll_balance_mccabe_reds, "McCabe_Tigers_Reds.json");
ll_balance_test!(test_ll_balance_mccabe_angels, "McCabe_Tigers_Angels.json");
ll_balance_test!(test_ll_balance_mccabe_yankees, "McCabe_Tigers_Yankees.json");
ll_balance_test!(test_ll_balance_mccabe_mets, "McCabe_Tigers_Mets.json");
ll_balance_test!(test_ll_balance_stars_tigers, "stars_vs_tigers_mar31.json");
ll_balance_test!(
    test_ll_balance_mariners_tigers_apr1,
    "mariners_vs_tigers_apr1.json"
);
ll_balance_test!(
    test_ll_balance_braves_cardinals_apr25,
    "10U_Braves_Cardinals_Apr25.json"
);
ll_balance_test!(
    test_ll_balance_cardinals_braves_apr25,
    "13U_Cardinals_Braves_Apr25.json"
);

// ---------------------------------------------------------------------------
// Undo/redo: Stars vs Tigers has 32 undos and 1 redo that restores a
// strikeout. Without redo support, the linescore is wrong (4-2 not 4-3).
// ---------------------------------------------------------------------------

#[test]
fn test_undo_redo_stars_tigers() {
    let json = include_str!("../testdata/stars_vs_tigers_mar31.json");
    let result = replay_from_json(json).expect("replay should succeed");

    // The redo restores a strikeout that is the 3rd out of an inning.
    // Without redo, Tigers get 2 runs. With redo, they get 3.
    let home_total: i32 = result.linescore_home.iter().sum();
    assert_eq!(
        home_total, 3,
        "Tigers should have 3 runs (redo restores a strikeout that shifts inning boundary)"
    );
    assert_eq!(
        result.away_pitching.outs_recorded, 15,
        "Away pitching should have 15 outs (5 full innings of Tigers batting)"
    );
}

// ---------------------------------------------------------------------------
// --no-steal-home simulation: scores should change for games with steals
// ---------------------------------------------------------------------------

#[test]
fn test_no_steal_home_reduces_runs() {
    let json = include_str!("../testdata/10U_Mariners_Cardinals.json");

    let normal = replay_from_json(json).expect("normal replay");
    let normal_away: i32 = normal.linescore_away.iter().sum();

    let simulated = replay_from_json_no_steal_home(json).expect("simulated replay");
    let sim_away: i32 = simulated.linescore_away.iter().sum();

    // Mariners had 6 steals of home. Simulation should produce fewer or equal runs.
    assert!(
        sim_away <= normal_away,
        "Simulated away runs ({sim_away}) should be <= normal ({normal_away})"
    );
    // Steals of home should be 0 in simulation
    assert_eq!(
        simulated.away_little_league.steals_of_home, 0,
        "No steals of home in simulation"
    );
    // PA invariants should still hold
    for ps in simulated.player_stats.values() {
        if ps.batting.pa > 0 {
            assert_eq!(
                ps.batting.ab
                    + ps.batting.bb
                    + ps.batting.hbp
                    + ps.batting.ci
                    + ps.batting.sac_fly
                    + ps.batting.sac_bunt,
                ps.batting.pa,
                "Player {} PA invariant failed in simulation",
                ps.player_id
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Scorebook edit semantics (edit_group / delete) + HBP pitch crediting.
//
// The edited_pitching_change fixtures are sanitized real books (see
// testdata/sanitize.py): every identifier remapped, timestamps rebased.
// Expected values are the provider's own aggregation of the same books
// (boxscore endpoint), mapped through the same id remapping.
// ---------------------------------------------------------------------------

fn pitching_line(result: &diamond_replay::GameResult, id: &str) -> (i32, i32, i32, i32) {
    let p = result.player_stats[id]
        .pitching
        .as_ref()
        .unwrap_or_else(|| panic!("no pitching stats for {id}"));
    (p.pitches, p.bf, p.k, p.outs_recorded)
}

/// One retroactive edit_group moves the relief pitcher's entry back to the
/// start of the 4th inning; two delete events remove pitcher_decision
/// sub-events. The game also contains hit-by-pitch deliveries (pseudo-pitch
/// with advancesCount:false + end_at_bat), which the provider counts as
/// pitches.
#[test]
fn test_edited_book_applies_retroactive_pitching_change() {
    let json = include_str!("../testdata/edited_pitching_change_a.json");
    let result = replay_from_json(json).expect("replay should succeed");

    // Home: starter through 3 innings, relief owns the 4th (39 pitches —
    // only ~5 without the edit applied).
    let starter = "1f315c05-8447-fbf6-5546-6b8161b17f48";
    let relief = "c7c80170-24ff-ee21-c15b-a35e5bf2aa77";
    assert_eq!(pitching_line(&result, starter), (53, 16, 6, 9));
    assert_eq!(pitching_line(&result, relief), (39, 10, 0, 3));

    // Away (no edits; one HBP delivery counted): provider says 45/9 + 66/15.
    assert_eq!(
        pitching_line(&result, "95098a11-2c59-a169-1774-69ecbced2a22"),
        (45, 9, 2, 3)
    );
    assert_eq!(
        pitching_line(&result, "fd870307-7cd3-a2da-66c2-7d84393efaa5"),
        (66, 15, 6, 9)
    );

    // Team aggregates must include the HBP deliveries (3 home, 1 away) and
    // agree with the per-player bags.
    assert_eq!(result.home_pitching.pitches, 92);
    assert_eq!(result.away_pitching.pitches, 111);

    // Team totals include the HBP deliveries (3 home, 1 away).
    let team_total = |team: &str| -> i32 {
        result
            .player_stats
            .values()
            .filter(|p| p.team_id == team && p.pitching.is_some())
            .map(|p| p.pitching.as_ref().unwrap().pitches)
            .sum()
    };
    assert_eq!(team_total(&result.home_id), 92);
    assert_eq!(team_total(&result.away_id), 111);
}

/// Two retroactive edit_groups: every pitching change in this book was
/// recorded after the fact. Without edit resolution the starter is credited
/// with all 120 pitches; the provider splits 46 / 53 / 21.
#[test]
fn test_edited_book_applies_two_retroactive_changes() {
    let json = include_str!("../testdata/edited_pitching_change_b.json");
    let result = replay_from_json(json).expect("replay should succeed");

    assert_eq!(
        pitching_line(&result, "65418bc3-6f9e-3a4e-dc60-b215b1ea0a52"),
        (46, 10, 6, 6)
    );
    assert_eq!(
        pitching_line(&result, "46d8113a-0e9a-343c-6a36-05600228078b"),
        (53, 10, 7, 7)
    );
    assert_eq!(
        pitching_line(&result, "81167c7d-d20c-a05c-0d0b-59527212a45c"),
        (21, 4, 1, 1)
    );
}

/// Control book: no edit events, no HBPs. Numbers must match the provider
/// exactly and stay identical to the pre-edit-resolution engine.
#[test]
fn test_clean_control_game_regression() {
    let json = include_str!("../testdata/clean_control_game.json");
    let result = replay_from_json(json).expect("replay should succeed");

    for (id, pitches, bf) in [
        ("7c008c5f-c33a-ab0c-89f0-be23589271ea", 72, 16),
        ("183df196-7eae-f8fa-88a7-113a1fe16944", 22, 6),
        ("d7cca6e7-018a-cb9f-dc4c-ab273e46ebb5", 21, 5),
        ("089fc3ec-2891-5903-f347-34b640bdb546", 47, 10),
        ("c64e1919-0038-0eb3-6945-7a05ae36e49f", 74, 15),
    ] {
        let line = pitching_line(&result, id);
        assert_eq!((line.0, line.1), (pitches, bf), "pitcher {id}");
    }
}

/// Both public entrypoints must run edit resolution — neither the standard
/// nor the options-based path may bypass it.
#[test]
fn test_edit_resolution_applies_through_both_entrypoints() {
    let json = include_str!("../testdata/edited_pitching_change_a.json");
    let relief = "c7c80170-24ff-ee21-c15b-a35e5bf2aa77";

    let standard = replay_from_json(json).expect("standard replay");
    assert_eq!(
        standard.player_stats[relief]
            .pitching
            .as_ref()
            .unwrap()
            .pitches,
        39
    );

    let simulated = replay_from_json_no_steal_home(json).expect("options replay");
    assert_eq!(
        simulated.player_stats[relief]
            .pitching
            .as_ref()
            .unwrap()
            .pitches,
        39
    );
}

// ---------------------------------------------------------------------------
// HBP pitch crediting: synthetic books.
// ---------------------------------------------------------------------------

fn hbp_book(prefix_events: Vec<serde_json::Value>) -> String {
    fn raw_event(seq: i64, event_data: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "id": format!("row-{seq}"),
            "stream_id": "test",
            "sequence_number": seq,
            "event_data": event_data.to_string(),
        })
    }
    let mut events = vec![
        raw_event(
            1,
            serde_json::json!({"code": "set_teams", "attributes": {"awayId": "away", "homeId": "home"}}),
        ),
        raw_event(
            2,
            serde_json::json!({"code": "fill_lineup_index", "attributes": {"teamId": "away", "playerId": "b1", "index": 0}}),
        ),
        raw_event(
            3,
            serde_json::json!({"code": "fill_lineup_index", "attributes": {"teamId": "away", "playerId": "b2", "index": 1}}),
        ),
        raw_event(
            4,
            serde_json::json!({"code": "fill_position", "attributes": {"teamId": "home", "playerId": "p1", "position": "P"}}),
        ),
    ];
    for (seq, ed) in (5..).zip(prefix_events) {
        events.push(raw_event(seq, ed));
    }
    serde_json::to_string(&events).unwrap()
}

fn pitch_ball() -> serde_json::Value {
    serde_json::json!({"code": "pitch", "attributes": {"result": "ball", "advancesCount": true}})
}

/// The provider encodes an HBP as a pseudo-pitch (advancesCount:false)
/// plus end_at_bat in one transaction.
fn hbp_transaction() -> serde_json::Value {
    serde_json::json!({"code": "transaction", "events": [
        {"code": "pitch", "attributes": {"result": "ball", "advancesCount": false, "advancesRunners": false}},
        {"code": "end_at_bat", "attributes": {"reason": "hit_by_pitch", "intentional": false}},
    ]})
}

/// HBP after n prior pitches: batter sees exactly n + 1 pitches, the
/// pitcher's count includes the HBP delivery, and ball/strike buckets are
/// untouched by it.
#[test]
fn test_hbp_delivery_counts_one_pitch() {
    let json = hbp_book(vec![pitch_ball(), pitch_ball(), hbp_transaction()]);
    let result = replay_from_json(&json).expect("replay should succeed");

    let batter = &result.player_stats["b1"];
    assert_eq!(batter.batting.pitches_seen, 3);
    assert_eq!(batter.batting.hbp, 1);

    let pitcher = result.player_stats["p1"].pitching.as_ref().unwrap();
    assert_eq!(pitcher.pitches, 3);
    assert_eq!(pitcher.balls, 2, "HBP delivery must not count as a ball");
    assert_eq!(pitcher.strikes_swinging, 0);
    assert_eq!(pitcher.strikes_looking, 0);
    assert_eq!(pitcher.fouls, 0);
    assert_eq!(pitcher.bf, 1);
}

/// A first-delivery HBP to a later batter starts that batter's
/// final_batter_start_pitch_count exactly like a first pitch would.
#[test]
fn test_first_delivery_hbp_sets_final_batter_start() {
    // Batter 1 walks on four pitches; batter 2 is hit by the first delivery.
    let json = hbp_book(vec![
        pitch_ball(),
        pitch_ball(),
        pitch_ball(),
        pitch_ball(),
        hbp_transaction(),
    ]);
    let result = replay_from_json(&json).expect("replay should succeed");

    let pitcher = result.player_stats["p1"].pitching.as_ref().unwrap();
    assert_eq!(pitcher.pitches, 5);
    assert_eq!(pitcher.final_batter_start_pitch_count, Some(5));
}

/// A pitching change immediately before an HBP: the incoming pitcher owns
/// the HBP delivery and starts their rest count at it.
#[test]
fn test_hbp_after_mid_pa_pitching_change() {
    let json = hbp_book(vec![
        pitch_ball(),
        pitch_ball(),
        serde_json::json!({"code": "fill_position", "attributes": {"teamId": "home", "playerId": "p2", "position": "P"}}),
        hbp_transaction(),
    ]);
    let result = replay_from_json(&json).expect("replay should succeed");

    let starter = result.player_stats["p1"].pitching.as_ref().unwrap();
    assert_eq!(starter.pitches, 2);
    let incoming = result.player_stats["p2"].pitching.as_ref().unwrap();
    assert_eq!(incoming.pitches, 1);
    assert_eq!(incoming.final_batter_start_pitch_count, Some(1));

    let batter = &result.player_stats["b1"];
    assert_eq!(batter.batting.pitches_seen, 3);
}
