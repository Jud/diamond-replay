use diamond_replay::filter::{NoStealHomeFilter, ReplayConfig};
use diamond_replay::{replay_from_json, replay_from_json_with_config};
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
                            + ps.batting.sac_fly + ps.batting.sac_bunt,
                        ps.batting.pa,
                        "{} player {} PA invariant failed: ab({}) + bb({}) + hbp({}) + sf({}) + sac({}) != pa({})",
                        $game_key, ps.player_id,
                        ps.batting.ab, ps.batting.bb, ps.batting.hbp,
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
                away_ll.runs_on_bip + away_ll.runs_passive, away_total,
                "{} away LL balance: bip({}) + passive({}) = {} != linescore({})",
                $file, away_ll.runs_on_bip, away_ll.runs_passive,
                away_ll.runs_on_bip + away_ll.runs_passive, away_total
            );
            assert_eq!(
                home_ll.runs_on_bip + home_ll.runs_passive, home_total,
                "{} home LL balance: bip({}) + passive({}) = {} != linescore({})",
                $file, home_ll.runs_on_bip, home_ll.runs_passive,
                home_ll.runs_on_bip + home_ll.runs_passive, home_total
            );
        }
    };
}

ll_balance_test!(test_ll_balance_mariners_cardinals, "10U_Mariners_Cardinals.json");
ll_balance_test!(test_ll_balance_mets_brewers, "10U_Mets_Brewers.json");
ll_balance_test!(test_ll_balance_braves_yankees, "10U_Braves_Yankees.json");
ll_balance_test!(test_ll_balance_tigers_dodgers, "10U_Tigers_Dodgers.json");
ll_balance_test!(test_ll_balance_13u_braves_padres, "13U_Braves_Padres.json");
ll_balance_test!(test_ll_balance_13u_mariners_brewers, "13U_Mariners_Brewers.json");
ll_balance_test!(test_ll_balance_13u_phillies_cardinals, "13U_Phillies_Cardinals.json");
ll_balance_test!(test_ll_balance_mccabe_reds, "McCabe_Tigers_Reds.json");
ll_balance_test!(test_ll_balance_mccabe_angels, "McCabe_Tigers_Angels.json");
ll_balance_test!(test_ll_balance_mccabe_yankees, "McCabe_Tigers_Yankees.json");
ll_balance_test!(test_ll_balance_mccabe_mets, "McCabe_Tigers_Mets.json");
ll_balance_test!(test_ll_balance_stars_tigers, "stars_vs_tigers_mar31.json");
ll_balance_test!(test_ll_balance_mariners_tigers_apr1, "mariners_vs_tigers_apr1.json");

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

    let mut config = ReplayConfig::default();
    config.filters.push(Box::new(NoStealHomeFilter));
    let simulated = replay_from_json_with_config(json, &config).expect("simulated replay");
    let sim_away: i32 = simulated.linescore_away.iter().sum();

    // Mariners had 6 steals of home. Simulation should produce fewer runs.
    assert!(
        sim_away < normal_away,
        "Simulated away runs ({sim_away}) should be less than normal ({normal_away})"
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
                ps.batting.ab + ps.batting.bb + ps.batting.hbp
                    + ps.batting.sac_fly + ps.batting.sac_bunt,
                ps.batting.pa,
                "Player {} PA invariant failed in simulation",
                ps.player_id
            );
        }
    }
}
