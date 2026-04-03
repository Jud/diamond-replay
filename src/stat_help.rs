/// Interactive stat help data for the TUI.
///
/// Each entry provides a description, formula, MLB benchmarks,
/// youth-specific context, and caveats for a single stat.
pub struct StatHelp {
    pub name: &'static str,
    pub description: &'static str,
    pub formula: &'static str,
    pub mlb_benchmark: &'static str,
    pub youth_context: &'static str,
    pub caveats: &'static str,
}

/// Column definitions per view: `(display_label, help_key)`.
pub const BOXSCORE_COLUMNS: &[(&str, &str)] = &[
    ("AB", "AB"),
    ("H", "H"),
    ("AVG", "AVG"),
    ("OBP", "OBP"),
    ("SLG", "SLG"),
    ("OPS", "OPS"),
    ("R", "R"),
    ("RBI", "RBI"),
    ("BB", "BB"),
    ("K", "K"),
    ("SB", "SB"),
];

pub const BATTING_COLUMNS: &[(&str, &str)] = &[
    ("PA", "PA"),
    ("wOBA", "wOBA"),
    ("ISO", "ISO"),
    ("BABIP", "BABIP"),
    ("K%", "K%_bat"),
    ("BB%", "BB%_bat"),
    ("QAB%", "QAB%"),
    ("P/PA", "P/PA"),
    ("GB%", "GB%"),
    ("SB%", "SB%"),
];

pub const PITCHING_COLUMNS: &[(&str, &str)] = &[
    ("IP", "IP"),
    ("ERA", "ERA"),
    ("FIP", "FIP"),
    ("WHIP", "WHIP"),
    ("K/9", "K/9"),
    ("BB/9", "BB/9"),
    ("K%", "K%_pitch"),
    ("K-BB%", "K-BB%"),
    ("CSW%", "CSW%"),
    ("FPS%", "FPS%"),
];

/// Look up help for a stat by its key.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn lookup(key: &str) -> Option<StatHelp> {
    let h = match key {
        // ── Box Score / Traditional Batting ──────────────────────
        "AB" => StatHelp {
            name: "At Bats",
            description: "Plate appearances minus walks, HBP, sacrifice flies, \
                          sacrifice bunts, and catcher interference.",
            formula: "PA - BB - HBP - SF - SAC - CI",
            mlb_benchmark: "~4.0 per game per starter",
            youth_context: "Lower than MLB due to shorter games and more walks.",
            caveats: "Does not reward plate discipline. A patient hitter \
                      who draws walks has fewer ABs.",
        },
        "H" => StatHelp {
            name: "Hits",
            description: "Singles + doubles + triples + home runs. The most \
                          basic measure of offensive production.",
            formula: "1B + 2B + 3B + HR",
            mlb_benchmark: "~1.2 H/game for an average hitter",
            youth_context: "Contact rates vary wildly by age group. Don't \
                            compare across divisions.",
            caveats: "Doesn't distinguish singles from extra-base hits. \
                      See ISO and SLG for power context.",
        },
        "AVG" => StatHelp {
            name: "Batting Average",
            description: "How often a batter gets a hit per at bat. The oldest \
                          and most recognized offensive stat.",
            formula: "H / AB",
            mlb_benchmark: "Average: .250 | Good: .280+ | Elite: .300+",
            youth_context: "Inflated by weak defense and high error rates. \
                            BABIP luck dominates in small samples.",
            caveats: "Ignores walks, HBP, and power. OBP and wOBA are \
                      better measures of offensive value.",
        },
        "OBP" => StatHelp {
            name: "On-Base Percentage",
            description: "How often a batter reaches base safely. Includes \
                          hits, walks, and hit-by-pitch.",
            formula: "(H + BB + HBP) / (AB + BB + HBP + SF)",
            mlb_benchmark: "Average: .320 | Good: .350+ | Elite: .380+",
            youth_context: "The most important traditional stat for youth \
                            hitters. Getting on base wins games.",
            caveats: "Treats all ways of reaching base equally. A walk \
                      and a home run count the same. wOBA fixes this.",
        },
        "SLG" => StatHelp {
            name: "Slugging Percentage",
            description: "Total bases per at bat. Measures raw power by \
                          weighting extra-base hits more heavily.",
            formula: "(1B + 2×2B + 3×3B + 4×HR) / AB",
            mlb_benchmark: "Average: .410 | Good: .470+ | Elite: .530+",
            youth_context: "Lower at young ages where extra-base power \
                            hasn't developed yet.",
            caveats: "Doesn't include walks. A power hitter who also \
                      walks a lot looks worse than he is.",
        },
        "OPS" => StatHelp {
            name: "On-Base Plus Slugging",
            description: "Quick composite of getting on base and hitting \
                          for power. Good at-a-glance offensive measure.",
            formula: "OBP + SLG",
            mlb_benchmark: "Average: .730 | Good: .800+ | Elite: .900+",
            youth_context: "Useful quick read, but mathematically imprecise. \
                            wOBA is the better composite.",
            caveats: "Adds two stats with different denominators, so it \
                      overvalues SLG relative to OBP.",
        },
        "R" => StatHelp {
            name: "Runs Scored",
            description: "Times the player crossed home plate.",
            formula: "",
            mlb_benchmark: "~0.5 per game for an average starter",
            youth_context: "Heavily influenced by teammates, batting order, \
                            and aggressive baserunning.",
            caveats: "Context-dependent. A great hitter on a weak team \
                      scores fewer runs. Not a good individual metric.",
        },
        "RBI" => StatHelp {
            name: "Runs Batted In",
            description: "Runs that scored as a direct result of the \
                          batter's plate appearance.",
            formula: "",
            mlb_benchmark: "~0.5 per game for an average starter",
            youth_context: "Mostly measures batting order position and \
                            who happens to be on base.",
            caveats: "Deeply context-dependent. Batting cleanup with \
                      runners on inflates RBI regardless of skill.",
        },
        "BB" => StatHelp {
            name: "Walks (Bases on Balls)",
            description: "Batter awarded first base on four balls. A measure \
                          of plate discipline and pitch recognition.",
            formula: "",
            mlb_benchmark: "~8-9% of PA league-wide",
            youth_context: "Very common in youth ball. Often reflects pitcher \
                            control more than batter discipline.",
            caveats: "In youth ball, separate earned walks (good eye) from \
                      gifted walks (wild pitcher). Context matters.",
        },
        "K" => StatHelp {
            name: "Strikeouts",
            description: "Batter out on three strikes, either looking \
                          (called) or swinging.",
            formula: "",
            mlb_benchmark: "Average: ~22% of PA",
            youth_context: "Raw count depends on plate appearances. Use K% \
                            for rate context across players.",
            caveats: "Raw count is misleading without PA context. A starter \
                      with 3 K in 4 PA is different from 3 K in 2 PA.",
        },
        "SB" => StatHelp {
            name: "Stolen Bases",
            description: "Successful stolen base attempts.",
            formula: "",
            mlb_benchmark: "20+ per season is above average",
            youth_context: "Speed and aggression dominate youth baserunning. \
                            Catcher arm strength is a big factor.",
            caveats: "Without caught-stealing data, SB alone doesn't show \
                      efficiency. See SB% for the full picture.",
        },

        // ── Advanced Batting ─────────────────────────────────────
        "PA" => StatHelp {
            name: "Plate Appearances",
            description: "Every completed trip to the plate. The denominator \
                          for all rate stats.",
            formula: "AB + BB + HBP + SF + SAC",
            mlb_benchmark: "~4.3 per game for a starter",
            youth_context: "Shorter games mean fewer PA. Small samples make \
                            all rate stats volatile.",
            caveats: "Be skeptical of rate stats from <10 PA. More PA = \
                      more reliable numbers.",
        },
        "wOBA" => StatHelp {
            name: "Weighted On-Base Average",
            description: "The gold standard offensive metric. Weights each \
                          outcome (walk, single, double, HR) by its actual \
                          run value.",
            formula: "(wBB×BB + wHBP×HBP + w1B×1B + w2B×2B + w3B×3B + wHR×HR)\n\
                      / (AB + BB + SF + HBP)",
            mlb_benchmark: "Average: .320 | Good: .350+ | Elite: .380+",
            youth_context: "Uses MLB linear weights by default. Rankings are \
                            still meaningful even if absolute values differ.",
            caveats: "Best single offensive number, but single-game wOBA \
                      is noisy. Needs ~50 PA to stabilize.",
        },
        "ISO" => StatHelp {
            name: "Isolated Power",
            description: "Measures raw extra-base power, independent of \
                          batting average. Pure power signal.",
            formula: "SLG - AVG  =  (2B + 2×3B + 3×HR) / AB",
            mlb_benchmark: "Average: .150 | Good: .190+ | Elite: .230+",
            youth_context: "Expect lower ISO in youth. Extra-base power \
                            develops with age and physical maturity.",
            caveats: "All-or-nothing hitters can have high ISO with low \
                      AVG. Pair with K% for a fuller picture.",
        },
        "BABIP" => StatHelp {
            name: "Batting Average on Balls In Play",
            description: "How often batted balls (excluding HR and K) fall \
                          for hits. The main luck indicator in batting stats.",
            formula: "(H - HR) / (AB - K - HR + SF)",
            mlb_benchmark: "Average: ~.300. Extremes regress toward .300.",
            youth_context: "Huge variance due to defense quality. A .450 \
                            BABIP in youth isn't lucky — the defense is bad.",
            caveats: "The #1 regression flag. Unsustainably high BABIP will \
                      come down; unsustainably low BABIP will rise.",
        },
        "K%_bat" => StatHelp {
            name: "Strikeout Rate",
            description: "Percentage of plate appearances ending in a \
                          strikeout. The key contact/swing-decision metric.",
            formula: "K / PA",
            mlb_benchmark: "Average: 22% | Good: <18% | Elite: <15%",
            youth_context: "Higher than MLB at all levels. Focus on the \
                            trend over time, not the absolute number.",
            caveats: "High K% with good QAB% means the batter is battling \
                      and competing, not just flailing.",
        },
        "BB%_bat" => StatHelp {
            name: "Walk Rate",
            description: "Percentage of plate appearances ending in a walk. \
                          Measures plate discipline and pitch recognition.",
            formula: "BB / PA",
            mlb_benchmark: "Average: 8.5% | Good: 10%+ | Elite: 14%+",
            youth_context: "Often reflects pitcher wildness more than batter \
                            eye. Separate earned walks from gifted walks.",
            caveats: "In youth, a team-wide high BB% usually means the \
                      opposing pitcher can't throw strikes.",
        },
        "QAB%" => StatHelp {
            name: "Quality At-Bat Percentage",
            description: "Percentage of PA that were 'quality' — hit, walk, \
                          sac, deep count (3+ pitches after 2 strikes), \
                          6+ pitch AB, or productive out.",
            formula: "QABs / PA",
            mlb_benchmark: "Not an MLB stat. Youth target: 60%+ is elite.",
            youth_context: "GameChanger's most promoted youth metric. Teams \
                            with 12+ QABs per game win 60%+ of the time.",
            caveats: "Youth-specific. Criteria vary by source. Our definition \
                      matches GameChanger's published criteria.",
        },
        "P/PA" => StatHelp {
            name: "Pitches Per Plate Appearance",
            description: "Average number of pitches seen per trip to the \
                          plate. Measures patience and approach quality.",
            formula: "pitches_seen / PA",
            mlb_benchmark: "Average: ~3.9 pitches per PA",
            youth_context: "Higher is generally better — seeing more pitches \
                            wears down the opposing pitcher.",
            caveats: "Can be high from fouling off pitches (good) or taking \
                      called strikes (bad). Context matters.",
        },
        "GB%" => StatHelp {
            name: "Ground Ball Percentage",
            description: "Percentage of balls in play that are ground balls.",
            formula: "ground_balls / BIP",
            mlb_benchmark: "Average: ~43%. Low GB% = more fly balls = more HR.",
            youth_context: "Batted ball mix develops with age and strength. \
                            Ground balls are harder to field in youth.",
            caveats: "For batters, high GB% limits power ceiling. For \
                      pitchers, high GB% suppresses damage.",
        },
        "SB%" => StatHelp {
            name: "Stolen Base Success Rate",
            description: "Percentage of stolen base attempts that succeed.",
            formula: "SB / (SB + CS)",
            mlb_benchmark: "Average: ~75%. Break-even: ~72%.",
            youth_context: "Speed dominates in youth. CS rate depends heavily \
                            on catcher arm and pitcher holding runners.",
            caveats: "Small sample in a single game. One failed attempt \
                      swings the rate dramatically.",
        },

        // ── Pitching ─────────────────────────────────────────────
        "IP" => StatHelp {
            name: "Innings Pitched",
            description: "Outs recorded divided by 3. Shown as whole innings \
                          plus thirds (e.g., 4.2 = 4 and 2/3 innings).",
            formula: "outs_recorded / 3",
            mlb_benchmark: "Starter: 6+ IP. Reliever: 1-2 IP.",
            youth_context: "Pitch count limits matter more than IP in youth. \
                            Many leagues cap pitches, not innings.",
            caveats: "Low IP inflates rate stats. A 0.2 IP outing with 3 ER \
                      gives a 40.50 ERA.",
        },
        "ERA" => StatHelp {
            name: "Earned Run Average",
            description: "Earned runs allowed per full game. The traditional \
                          measure of pitcher effectiveness.",
            formula: "(ER / IP) × innings_per_game",
            mlb_benchmark: "Average: 4.00 | Good: <3.50 | Elite: <2.50",
            youth_context: "Heavily skewed by defense quality. FIP is a much \
                            more reliable pitcher evaluation tool.",
            caveats: "Earned/unearned split depends on scorer judgment. \
                      Small IP makes ERA extremely volatile.",
        },
        "FIP" => StatHelp {
            name: "Fielding Independent Pitching",
            description: "What ERA should be based only on outcomes the \
                          pitcher controls: strikeouts, walks, and home runs.",
            formula: "((13×HR) + (3×(BB+HBP)) - (2×K)) / IP + constant",
            mlb_benchmark: "Average: 4.00 | Good: <3.50 | Elite: <2.80",
            youth_context: "Critical for youth where defense is unreliable. \
                            Best single pitching evaluation metric.",
            caveats: "Ignores batted ball quality. A pitcher who induces \
                      weak contact looks worse in FIP than he is.",
        },
        "WHIP" => StatHelp {
            name: "Walks + Hits per Inning Pitched",
            description: "Baserunners allowed per inning. Lower is better. \
                          Simple measure of pitcher control.",
            formula: "(BB + H) / IP",
            mlb_benchmark: "Average: 1.30 | Good: <1.15 | Elite: <1.00",
            youth_context: "Walks drive WHIP in youth. A pitcher with low \
                            WHIP is controlling the strike zone.",
            caveats: "Treats walks and hits equally. A walk is usually \
                      worse since no out was recorded.",
        },
        "K/9" => StatHelp {
            name: "Strikeouts per 9 Innings",
            description: "Strikeout rate normalized to a full 9-inning game.",
            formula: "(K / IP) × 9",
            mlb_benchmark: "Average: 8.5 | Good: 9.5+ | Elite: 11.0+",
            youth_context: "Normalized to 9 innings even for shorter games. \
                            K% is preferred for direct comparison.",
            caveats: "K% (K/BF) is more stable. K/9 inflates with low IP \
                      because the denominator is small.",
        },
        "BB/9" => StatHelp {
            name: "Walks per 9 Innings",
            description: "Walk rate normalized to a 9-inning game. Lower \
                          is better. Measures control.",
            formula: "(BB / IP) × 9",
            mlb_benchmark: "Average: 3.2 | Good: <2.8 | Elite: <2.0",
            youth_context: "Youth pitchers walk many more batters. Track the \
                            trend over multiple outings.",
            caveats: "Same small-IP inflation problem as K/9. BB% (BB/BF) \
                      is more reliable.",
        },
        "K%_pitch" => StatHelp {
            name: "Strikeout Rate",
            description: "Percentage of batters faced who struck out. The \
                          preferred measure of a pitcher's strikeout ability.",
            formula: "K / BF",
            mlb_benchmark: "Average: 22% | Good: 25%+ | Elite: 30%+",
            youth_context: "Preferred over K/9 because it directly measures \
                            dominance per batter faced.",
            caveats: "Small BF samples in relief outings make this volatile. \
                      Look for multi-game trends.",
        },
        "K-BB%" => StatHelp {
            name: "Strikeout Minus Walk Rate",
            description: "The single most predictive pitching metric. Can \
                          you strike batters out without walking them?",
            formula: "K% - BB%",
            mlb_benchmark: "Average: 12% | Good: 15%+ | Elite: 20%+",
            youth_context: "The best quick evaluation of a youth pitcher. \
                            Positive and growing = pitcher is developing.",
            caveats: "Doesn't capture HR or contact quality. Combine with \
                      FIP for the full picture.",
        },
        "CSW%" => StatHelp {
            name: "Called Strike + Whiff Rate",
            description: "Percentage of pitches that are called strikes or \
                          swinging strikes. Measures stuff and deception.",
            formula: "(called_strikes + swinging_strikes) / total_pitches",
            mlb_benchmark: "Average: 27-28% | Elite: 30%+",
            youth_context: "Best pitch-level metric available. High CSW% = \
                            the stuff is working. Track per outing.",
            caveats: "Doesn't account for what happens when the ball is \
                      put in play. Pair with BABIP and FIP.",
        },
        "FPS%" => StatHelp {
            name: "First Pitch Strike Percentage",
            description: "How often the pitcher starts the at-bat with a \
                          strike. The foundation of pitching development.",
            formula: "first_pitch_strikes / batters_faced",
            mlb_benchmark: "Average: ~60% | Target: 65%+",
            youth_context: "GameChanger's #1 recommended pitcher development \
                            stat. Target: 60%+. Getting ahead changes everything.",
            caveats: "A first-pitch strike could be a meatball that gets \
                      crushed. The outcome of that pitch matters too.",
        },

        _ => return None,
    };
    Some(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_known_stats() {
        assert!(lookup("AVG").is_some());
        assert!(lookup("wOBA").is_some());
        assert!(lookup("FIP").is_some());
        assert!(lookup("K%_bat").is_some());
        assert!(lookup("K%_pitch").is_some());
        assert_eq!(lookup("AVG").unwrap().name, "Batting Average");
    }

    #[test]
    fn lookup_unknown_returns_none() {
        assert!(lookup("XYZZY").is_none());
    }

    #[test]
    fn all_column_keys_resolve() {
        for (_label, key) in BOXSCORE_COLUMNS
            .iter()
            .chain(BATTING_COLUMNS)
            .chain(PITCHING_COLUMNS)
        {
            assert!(
                lookup(key).is_some(),
                "missing help entry for column key: {key}"
            );
        }
    }
}
