use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use diamond_replay::{
    replay_from_json, replay_from_json_no_steal_home, replay_from_json_with_options, BattingStats,
    GameResult, LittleLeagueStats, PitchingStats, PlayerGameStats, ReplayOptions,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct ExpectedBoxScore {
    away: Vec<i32>,
    home: Vec<i32>,
    #[serde(rename = "awayTotal")]
    away_total: i32,
    #[serde(rename = "homeTotal")]
    home_total: i32,
}

fn testdata_path(file_name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testdata")
        .join(file_name)
}

fn load_box_scores() -> BTreeMap<String, ExpectedBoxScore> {
    let path = testdata_path("box_scores.json");
    let json = fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    serde_json::from_str(&json)
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()))
}

fn load_game(game_key: &str) -> String {
    let path = testdata_path(&format!("{game_key}.json"));
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read fixture {}: {err}", path.display()))
}

#[test]
fn ground_truth_games_pass_standard_replay_gates() {
    for (game_key, expected) in load_box_scores() {
        let json = load_game(&game_key);
        let legacy = replay_from_json(&json)
            .unwrap_or_else(|err| panic!("{game_key}: standard replay failed: {err}"));
        let result = replay_from_json_with_options(&json, ReplayOptions::standard())
            .unwrap_or_else(|err| panic!("{game_key}: options replay failed: {err}"));

        assert_same_replay(&game_key, "standard options replay", &legacy, &result);
        assert_box_score(&game_key, &result, &expected);
        assert_core_replay_invariants(&game_key, &result);
        assert_standard_team_totals(&game_key, &result);
        assert_serializes(&game_key, &result);
    }
}

#[test]
fn ground_truth_games_pass_no_steal_home_simulation_gates() {
    for (game_key, _expected) in load_box_scores() {
        let json = load_game(&game_key);
        let standard = replay_from_json_with_options(&json, ReplayOptions::standard())
            .unwrap_or_else(|err| panic!("{game_key}: standard options replay failed: {err}"));
        let legacy_simulated = replay_from_json_no_steal_home(&json)
            .unwrap_or_else(|err| panic!("{game_key}: no-steal-home replay failed: {err}"));
        let simulated = replay_from_json_with_options(&json, ReplayOptions::no_steal_home())
            .unwrap_or_else(|err| panic!("{game_key}: no-steal-home options replay failed: {err}"));

        assert_same_replay(
            &game_key,
            "no-steal-home options replay",
            &legacy_simulated,
            &simulated,
        );
        assert_core_replay_invariants(&game_key, &simulated);
        assert_standard_team_totals(&game_key, &simulated);
        assert_serializes(&game_key, &simulated);

        assert_eq!(
            simulated.away_little_league.steals_of_home, 0,
            "{game_key}: away steals of home should be fully suppressed"
        );
        assert_eq!(
            simulated.home_little_league.steals_of_home, 0,
            "{game_key}: home steals of home should be fully suppressed"
        );

        let standard_away_runs: i32 = standard.linescore_away.iter().sum();
        let standard_home_runs: i32 = standard.linescore_home.iter().sum();
        let simulated_away_runs: i32 = simulated.linescore_away.iter().sum();
        let simulated_home_runs: i32 = simulated.linescore_home.iter().sum();

        assert!(
            simulated_away_runs <= standard_away_runs,
            "{game_key}: no-steal-home away runs ({simulated_away_runs}) exceeded standard runs ({standard_away_runs})"
        );
        assert!(
            simulated_home_runs <= standard_home_runs,
            "{game_key}: no-steal-home home runs ({simulated_home_runs}) exceeded standard runs ({standard_home_runs})"
        );
    }
}

fn assert_same_replay(game_key: &str, label: &str, expected: &GameResult, actual: &GameResult) {
    assert_eq!(actual.home_id, expected.home_id, "{game_key}: {label} home");
    assert_eq!(actual.away_id, expected.away_id, "{game_key}: {label} away");
    assert_eq!(
        actual.linescore_away, expected.linescore_away,
        "{game_key}: {label} away linescore"
    );
    assert_eq!(
        actual.linescore_home, expected.linescore_home,
        "{game_key}: {label} home linescore"
    );
    assert_eq!(
        actual.first_timestamp, expected.first_timestamp,
        "{game_key}: {label} first timestamp"
    );
    assert_eq!(
        actual.first_pitch_timestamp, expected.first_pitch_timestamp,
        "{game_key}: {label} first pitch timestamp"
    );
    assert_eq!(
        actual.last_pitch_timestamp, expected.last_pitch_timestamp,
        "{game_key}: {label} last pitch timestamp"
    );
    assert_eq!(
        actual.last_timestamp, expected.last_timestamp,
        "{game_key}: {label} last timestamp"
    );
    assert_eq!(
        actual.transition_gaps, expected.transition_gaps,
        "{game_key}: {label} transition gaps"
    );
    assert_eq!(
        actual.dead_time_per_inning, expected.dead_time_per_inning,
        "{game_key}: {label} dead time"
    );
    assert_eq!(
        actual.inning_durations, expected.inning_durations,
        "{game_key}: {label} inning durations"
    );

    assert_same_json(
        game_key,
        label,
        "away batting",
        &expected.away_batting,
        &actual.away_batting,
    );
    assert_same_json(
        game_key,
        label,
        "home batting",
        &expected.home_batting,
        &actual.home_batting,
    );
    assert_same_json(
        game_key,
        label,
        "away pitching",
        &expected.away_pitching,
        &actual.away_pitching,
    );
    assert_same_json(
        game_key,
        label,
        "home pitching",
        &expected.home_pitching,
        &actual.home_pitching,
    );
    assert_same_json(
        game_key,
        label,
        "away little league",
        &expected.away_little_league,
        &actual.away_little_league,
    );
    assert_same_json(
        game_key,
        label,
        "home little league",
        &expected.home_little_league,
        &actual.home_little_league,
    );

    let mut expected_players = expected.player_stats.keys().collect::<Vec<_>>();
    let mut actual_players = actual.player_stats.keys().collect::<Vec<_>>();
    expected_players.sort_unstable();
    actual_players.sort_unstable();
    assert_eq!(
        actual_players, expected_players,
        "{game_key}: {label} player keys"
    );

    for player_id in expected_players {
        let expected_player = expected
            .player_stats
            .get(player_id)
            .expect("expected player key came from map");
        let actual_player = actual
            .player_stats
            .get(player_id)
            .expect("actual player key should exist after key comparison");
        assert_same_json(
            game_key,
            label,
            &format!("player {player_id}"),
            expected_player,
            actual_player,
        );
    }
}

fn assert_same_json<T: Serialize>(
    game_key: &str,
    label: &str,
    section: &str,
    expected: &T,
    actual: &T,
) {
    let mut expected_json = serde_json::to_value(expected)
        .unwrap_or_else(|err| panic!("{game_key}: failed to serialize expected {section}: {err}"));
    let mut actual_json = serde_json::to_value(actual)
        .unwrap_or_else(|err| panic!("{game_key}: failed to serialize actual {section}: {err}"));
    normalize_order_insensitive_arrays(&mut expected_json);
    normalize_order_insensitive_arrays(&mut actual_json);

    assert_eq!(
        actual_json, expected_json,
        "{game_key}: {label} {section} diverged from legacy path"
    );
}

fn normalize_order_insensitive_arrays(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                if key == "spray_chart" {
                    if let serde_json::Value::Array(entries) = child {
                        for entry in entries.iter_mut() {
                            normalize_order_insensitive_arrays(entry);
                        }
                        entries.sort_by_key(|entry| entry.to_string());
                    }
                } else {
                    normalize_order_insensitive_arrays(child);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                normalize_order_insensitive_arrays(item);
            }
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
    }
}

fn assert_box_score(game_key: &str, result: &GameResult, expected: &ExpectedBoxScore) {
    assert_eq!(
        result.linescore_away, expected.away,
        "{game_key}: away linescore mismatch"
    );
    assert_eq!(
        result.linescore_home, expected.home,
        "{game_key}: home linescore mismatch"
    );
    assert_eq!(
        result.linescore_away.iter().sum::<i32>(),
        expected.away_total,
        "{game_key}: away total mismatch"
    );
    assert_eq!(
        result.linescore_home.iter().sum::<i32>(),
        expected.home_total,
        "{game_key}: home total mismatch"
    );
}

fn assert_core_replay_invariants(game_key: &str, result: &GameResult) {
    let away_runs: i32 = result.linescore_away.iter().sum();
    let home_runs: i32 = result.linescore_home.iter().sum();

    assert_little_league_balance(game_key, "away", away_runs, &result.away_little_league);
    assert_little_league_balance(game_key, "home", home_runs, &result.home_little_league);

    for player in result.player_stats.values() {
        assert_batting_invariants(game_key, player);
        if let Some(pitching) = &player.pitching {
            assert_pitching_invariants(game_key, &player.player_id, pitching);
        }
    }

    assert_non_negative_batting(game_key, "away team", &result.away_batting);
    assert_non_negative_batting(game_key, "home team", &result.home_batting);
    assert_non_negative_pitching(game_key, "away team", &result.away_pitching);
    assert_non_negative_pitching(game_key, "home team", &result.home_pitching);
    assert_non_negative_little_league(game_key, "away team", &result.away_little_league);
    assert_non_negative_little_league(game_key, "home team", &result.home_little_league);
}

fn assert_standard_team_totals(game_key: &str, result: &GameResult) {
    assert_team_batting_totals(
        game_key,
        "away",
        &result.away_id,
        &result.away_batting,
        &result.player_stats,
    );
    assert_team_batting_totals(
        game_key,
        "home",
        &result.home_id,
        &result.home_batting,
        &result.player_stats,
    );
    assert_team_pitching_totals(
        game_key,
        "away",
        &result.away_id,
        &result.away_pitching,
        &result.player_stats,
    );
    assert_team_pitching_totals(
        game_key,
        "home",
        &result.home_id,
        &result.home_pitching,
        &result.player_stats,
    );

    assert_eq!(
        result.away_pitching.bf, result.home_batting.pa,
        "{game_key}: away pitching BF should equal home batting PA"
    );
    assert_eq!(
        result.home_pitching.bf, result.away_batting.pa,
        "{game_key}: home pitching BF should equal away batting PA"
    );
}

fn assert_team_batting_totals(
    game_key: &str,
    side: &str,
    team_id: &str,
    totals: &BattingStats,
    player_stats: &BTreeComparablePlayerStats,
) {
    let mut summed = BattingStats::default();
    for player in player_stats
        .values()
        .filter(|player| player.team_id == team_id)
    {
        summed.pa += player.batting.pa;
        summed.ab += player.batting.ab;
        summed.hits += player.batting.hits;
        summed.bb += player.batting.bb;
        summed.hbp += player.batting.hbp;
        summed.ci += player.batting.ci;
        summed.k += player.batting.k;
        summed.runs += player.batting.runs;
        summed.rbi += player.batting.rbi;
    }

    assert_eq!(summed.pa, totals.pa, "{game_key}: {side} batting PA total");
    assert_eq!(summed.ab, totals.ab, "{game_key}: {side} batting AB total");
    assert_eq!(
        summed.hits, totals.hits,
        "{game_key}: {side} batting hits total"
    );
    assert_eq!(summed.bb, totals.bb, "{game_key}: {side} batting BB total");
    assert_eq!(
        summed.hbp, totals.hbp,
        "{game_key}: {side} batting HBP total"
    );
    assert_eq!(summed.ci, totals.ci, "{game_key}: {side} batting CI total");
    assert_eq!(summed.k, totals.k, "{game_key}: {side} batting K total");
    assert_eq!(
        summed.runs, totals.runs,
        "{game_key}: {side} batting runs total"
    );
    assert_eq!(
        summed.rbi, totals.rbi,
        "{game_key}: {side} batting RBI total"
    );
}

fn assert_team_pitching_totals(
    game_key: &str,
    side: &str,
    team_id: &str,
    totals: &PitchingStats,
    player_stats: &BTreeComparablePlayerStats,
) {
    let mut summed = PitchingStats::default();
    for pitching in player_stats
        .values()
        .filter(|player| player.team_id == team_id)
        .filter_map(|player| player.pitching.as_ref())
    {
        summed.pitches += pitching.pitches;
        summed.balls += pitching.balls;
        summed.k += pitching.k;
        summed.bb += pitching.bb;
        summed.hbp += pitching.hbp;
        summed.hits_allowed += pitching.hits_allowed;
        summed.hr_allowed += pitching.hr_allowed;
        summed.runs_allowed += pitching.runs_allowed;
        summed.earned_runs_allowed += pitching.earned_runs_allowed;
        summed.outs_recorded += pitching.outs_recorded;
        summed.bf += pitching.bf;
        summed.bip += pitching.bip;
    }

    assert_eq!(
        summed.pitches, totals.pitches,
        "{game_key}: {side} pitching pitches total"
    );
    assert_eq!(
        summed.balls, totals.balls,
        "{game_key}: {side} pitching balls total"
    );
    assert_eq!(summed.k, totals.k, "{game_key}: {side} pitching K total");
    assert_eq!(summed.bb, totals.bb, "{game_key}: {side} pitching BB total");
    assert_eq!(
        summed.hbp, totals.hbp,
        "{game_key}: {side} pitching HBP total"
    );
    assert_eq!(
        summed.hits_allowed, totals.hits_allowed,
        "{game_key}: {side} pitching hits allowed total"
    );
    assert_eq!(
        summed.hr_allowed, totals.hr_allowed,
        "{game_key}: {side} pitching HR allowed total"
    );
    assert_eq!(
        summed.runs_allowed, totals.runs_allowed,
        "{game_key}: {side} pitching runs allowed total"
    );
    assert_eq!(
        summed.earned_runs_allowed, totals.earned_runs_allowed,
        "{game_key}: {side} pitching earned runs allowed total"
    );
    assert_eq!(
        summed.outs_recorded, totals.outs_recorded,
        "{game_key}: {side} pitching outs total"
    );
    assert_eq!(summed.bf, totals.bf, "{game_key}: {side} pitching BF total");
    assert_eq!(
        summed.bip, totals.bip,
        "{game_key}: {side} pitching BIP total"
    );
}

type BTreeComparablePlayerStats = std::collections::HashMap<String, PlayerGameStats>;

fn assert_batting_invariants(game_key: &str, player: &PlayerGameStats) {
    let b = &player.batting;
    assert_non_negative_batting(game_key, &player.player_id, b);

    assert_eq!(
        b.ab + b.bb + b.hbp + b.ci + b.sac_fly + b.sac_bunt,
        b.pa,
        "{game_key}: player {} PA invariant failed",
        player.player_id
    );
    assert_eq!(
        b.hits,
        b.singles + b.doubles + b.triples + b.home_runs,
        "{game_key}: player {} hit invariant failed",
        player.player_id
    );
    assert_eq!(
        b.tb,
        b.singles + 2 * b.doubles + 3 * b.triples + 4 * b.home_runs,
        "{game_key}: player {} total bases invariant failed",
        player.player_id
    );
    assert_eq!(
        b.xbh,
        b.doubles + b.triples + b.home_runs,
        "{game_key}: player {} extra-base hits invariant failed",
        player.player_id
    );
}

fn assert_pitching_invariants(game_key: &str, player_id: &str, pitching: &PitchingStats) {
    assert_non_negative_pitching(game_key, player_id, pitching);
    let expected_ip = expected_ip_display(pitching.outs_recorded);
    assert_eq!(
        pitching.ip_display.as_deref(),
        Some(expected_ip.as_str()),
        "{game_key}: pitcher {player_id} IP display mismatch"
    );
    if let Some(ip) = pitching.ip {
        let expected = f64::from(pitching.outs_recorded) / 3.0;
        assert!(
            (ip - expected).abs() < f64::EPSILON,
            "{game_key}: pitcher {player_id} IP value mismatch: got {ip}, expected {expected}"
        );
    }
    assert!(
        pitching.strikes_swinging + pitching.strikes_looking + pitching.fouls <= pitching.pitches,
        "{game_key}: pitcher {player_id} has more tracked strikes/fouls than pitches"
    );
}

fn assert_little_league_balance(
    game_key: &str,
    side: &str,
    runs_total: i32,
    ll: &LittleLeagueStats,
) {
    assert_eq!(
        ll.runs_on_bip + ll.runs_passive,
        runs_total,
        "{game_key}: {side} LL run balance failed"
    );
}

fn assert_serializes(game_key: &str, result: &GameResult) {
    serde_json::to_value(result)
        .unwrap_or_else(|err| panic!("{game_key}: GameResult JSON serialization failed: {err}"));
}

fn expected_ip_display(outs: i32) -> String {
    format!("{}.{}", outs / 3, outs % 3)
}

fn assert_non_negative_batting(game_key: &str, label: &str, b: &BattingStats) {
    for (field, value) in [
        ("pa", b.pa),
        ("ab", b.ab),
        ("k", b.k),
        ("k_looking", b.k_looking),
        ("k_swinging", b.k_swinging),
        ("bb", b.bb),
        ("hbp", b.hbp),
        ("ci", b.ci),
        ("singles", b.singles),
        ("doubles", b.doubles),
        ("triples", b.triples),
        ("home_runs", b.home_runs),
        ("sac_fly", b.sac_fly),
        ("sac_bunt", b.sac_bunt),
        ("fc", b.fc),
        ("roe", b.roe),
        ("gidp", b.gidp),
        ("rbi", b.rbi),
        ("runs", b.runs),
        ("ground_balls", b.ground_balls),
        ("fly_balls", b.fly_balls),
        ("line_drives", b.line_drives),
        ("pop_ups", b.pop_ups),
        ("hard_hit_balls", b.hard_hit_balls),
        ("pitches_seen", b.pitches_seen),
        ("qab", b.qab),
        ("competitive_ab", b.competitive_ab),
        ("sb", b.sb),
        ("cs", b.cs),
        ("hits", b.hits),
        ("tb", b.tb),
        ("xbh", b.xbh),
    ] {
        assert!(
            value >= 0,
            "{game_key}: {label} batting field {field} was negative: {value}"
        );
    }
}

fn assert_non_negative_pitching(game_key: &str, label: &str, p: &PitchingStats) {
    for (field, value) in [
        ("pitches", p.pitches),
        ("balls", p.balls),
        ("strikes_swinging", p.strikes_swinging),
        ("strikes_looking", p.strikes_looking),
        ("fouls", p.fouls),
        ("k", p.k),
        ("bb", p.bb),
        ("hbp", p.hbp),
        ("hits_allowed", p.hits_allowed),
        ("hr_allowed", p.hr_allowed),
        ("runs_allowed", p.runs_allowed),
        ("earned_runs_allowed", p.earned_runs_allowed),
        ("outs_recorded", p.outs_recorded),
        ("bf", p.bf),
        ("bip", p.bip),
        ("ground_balls", p.ground_balls),
        ("fly_balls", p.fly_balls),
        ("line_drives", p.line_drives),
        ("pop_ups", p.pop_ups),
        ("first_pitch_strikes", p.first_pitch_strikes),
        ("wp", p.wp),
    ] {
        assert!(
            value >= 0,
            "{game_key}: {label} pitching field {field} was negative: {value}"
        );
    }
}

fn assert_non_negative_little_league(game_key: &str, label: &str, ll: &LittleLeagueStats) {
    for (field, value) in [
        ("runs_on_bip", ll.runs_on_bip),
        ("runs_passive", ll.runs_passive),
        ("wp", ll.wp),
        ("pb", ll.pb),
        ("cs", ll.cs),
        ("steals_of_home", ll.steals_of_home),
        ("bb_loaded", ll.bb_loaded),
        ("hbp_loaded", ll.hbp_loaded),
    ] {
        assert!(
            value >= 0,
            "{game_key}: {label} little-league field {field} was negative: {value}"
        );
    }

    for value in &ll.pitches_between_bip {
        assert!(
            *value >= 0,
            "{game_key}: {label} pitches_between_bip contained a negative value: {value}"
        );
    }
    for value in &ll.pitches_between_bip_pitching {
        assert!(
            *value >= 0,
            "{game_key}: {label} pitches_between_bip_pitching contained a negative value: {value}"
        );
    }
}
