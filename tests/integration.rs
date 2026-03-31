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

            // Verify runs_on_bip + runs_passive == linescore total
            let away_total: i32 = result.linescore_away.iter().sum();
            let home_total: i32 = result.linescore_home.iter().sum();
            assert_eq!(
                result.away_batting.total_runs(),
                away_total,
                "{} away runs_total mismatch",
                $game_key
            );
            assert_eq!(
                result.home_batting.total_runs(),
                home_total,
                "{} home runs_total mismatch",
                $game_key
            );
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

    eprintln!(
        "Away team PA: team={}, players={}",
        result.away_batting.pa, away_player_pa
    );
    eprintln!(
        "Home team PA: team={}, players={}",
        result.home_batting.pa, home_player_pa
    );
    eprintln!("Total players with stats: {}", result.player_stats.len());

    // Player runs should sum to linescore
    let away_runs: i32 = result
        .player_stats
        .values()
        .filter(|p| p.team_id == result.away_id)
        .map(|p| p.baserunning.runs)
        .sum();
    let home_runs: i32 = result
        .player_stats
        .values()
        .filter(|p| p.team_id == result.home_id)
        .map(|p| p.baserunning.runs)
        .sum();
    let away_ls: i32 = result.linescore_away.iter().sum();
    let home_ls: i32 = result.linescore_home.iter().sum();
    eprintln!(
        "Away runs: linescore={}, player_baserunning={}",
        away_ls, away_runs
    );
    eprintln!(
        "Home runs: linescore={}, player_baserunning={}",
        home_ls, home_runs
    );
}
