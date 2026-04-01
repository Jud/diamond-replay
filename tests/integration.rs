use diamond_replay::replay_from_json;
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
