# Diamond Replay Statistics Reference

This document defines every statistic that diamond-replay computes. Stats are
organized by category. Each entry includes the abbreviation, full name,
formula, and what raw data is required.

All stats are computed per-player per-game unless noted otherwise.

---

## Batting - Traditional

### AB - At Bats
PA minus walks, HBP, sacrifice flies, sacrifice bunts, and catcher interference.

    AB = PA - BB - HBP - SF - SAC - CI

### H - Hits
Singles + Doubles + Triples + Home Runs.

### TB - Total Bases
    TB = 1B + (2 * 2B) + (3 * 3B) + (4 * HR)

### AVG - Batting Average
    AVG = H / AB

### OBP - On-Base Percentage
    OBP = (H + BB + HBP) / (AB + BB + HBP + SF)

### SLG - Slugging Percentage
    SLG = TB / AB

### OPS - On-Base Plus Slugging
    OPS = OBP + SLG

### XBH - Extra-Base Hits
    XBH = 2B + 3B + HR

### RBI - Runs Batted In
Runs that score as a direct result of the batter's plate appearance. Credited
on hits, walks with bases loaded, HBP with bases loaded, sac flies, sac bunts,
fielder's choice groundouts, and reached-on-error when a run scores. Not
credited on double plays.

Requires: tracking which runners score on each play and attributing to batter.

### R - Runs Scored
Times the player crosses home plate.

### PA - Plate Appearances
Every completed trip to the plate.

### BB - Walks (Bases on Balls)
Four balls in an at-bat.

### K - Strikeouts
Three strikes. Tracked as total, looking (called), and swinging.

### HBP - Hit By Pitch
Batter awarded first base after being hit by a pitch.

### SF - Sacrifice Fly
Fly ball out that scores a runner from third.

### SAC - Sacrifice Bunt
Bunt that advances a runner while the batter is retired.

### FC - Fielder's Choice
Batter reaches base because a fielder chose to retire a different runner.

### ROE - Reached On Error
Batter reaches base due to a fielding error.

### GIDP - Grounded Into Double Play
Batter hits into a double play.

Requires: tracking double-play events attributed to the batter.

---

## Batting - Sabermetric

### ISO - Isolated Power
Measures raw extra-base power, independent of batting average.

    ISO = SLG - AVG

Equivalently: `(2B + 2*3B + 3*HR) / AB`

### BABIP - Batting Average on Balls In Play
How often batted balls (excluding HR and K) fall for hits. League average is
typically around .300. Extremes suggest luck regression.

    BABIP = (H - HR) / (AB - K - HR + SF)

### K% - Strikeout Rate
    K% = K / PA

### BB% - Walk Rate
    BB% = BB / PA

### BB/K - Walk-to-Strikeout Ratio
    BB/K = BB / K

### wOBA - Weighted On-Base Average
The gold standard offensive metric. Weights each outcome by its actual run
value rather than treating all hits equally.

    wOBA = (wBB*BB + wHBP*HBP + w1B*1B + w2B*2B + w3B*3B + wHR*HR)
           / (AB + BB + SF + HBP)

Default linear weights (MLB 2023 approximation):
- wBB = 0.690
- wHBP = 0.720
- w1B = 0.880
- w2B = 1.245
- w3B = 1.575
- wHR = 2.015

For youth leagues, weights should be calibrated from accumulated game data.

### wRC+ - Weighted Runs Created Plus
wOBA normalized to league average and adjusted for context. 100 = league
average; 150 means 50% better than average.

    wRAA = ((wOBA - lgwOBA) / wOBA_scale) * PA
    wRC+ = (((wRAA/PA + lgR/PA) + (lgR/PA - PF*lgR/PA)) / (lgwRC/lgPA)) * 100

Requires: league-wide wOBA baseline, league R/PA, wOBA scale constant, park
factor (default 1.0 for neutral).

### HR/FB - Home Run to Fly Ball Rate
    HR/FB = HR / FB

Requires: fly ball count per batter.

### GB%, FB%, LD% - Batted Ball Distribution
Percentage of balls in play that are ground balls, fly balls, or line drives.

    GB% = ground_balls / BIP
    FB% = fly_balls / BIP
    LD% = line_drives / BIP

Requires: batted ball type classification (we parse BipPlayType on every BIP).

---

## Batting - Youth & Process

### QAB - Quality At-Bat
A plate appearance that meets ANY of these criteria:
- Hit (single, double, triple, HR)
- Walk (BB) or Hit-By-Pitch (HBP)
- Sacrifice bunt or sacrifice fly
- Reached on error
- Saw 3+ pitches after reaching a 2-strike count
- Saw 6+ total pitches in the at-bat
- Advanced a runner (productive out) - tracked via BR events after BIP

    QAB% = QABs / PA

GameChanger's most promoted youth stat. 60%+ is elite; teams with 12+ QABs per
game win 60%+ of the time.

Requires: pitch count per PA, count progression, play result.

### Competitive AB%
Percentage of plate appearances where the batter reached a 2-strike count
(showed willingness to compete / battle).

    Competitive% = PAs_reaching_2_strikes / PA

### P/PA - Pitches Per Plate Appearance
    P/PA = pitches_seen / PA

Measures batting approach and patience. Higher = seeing more pitches.

---

## Pitching - Traditional

### IP - Innings Pitched
    IP = outs_recorded / 3

Displayed as whole innings plus thirds (e.g., 6.2 = 6 and 2/3 innings).

### ERA - Earned Run Average
    ERA = (ER / IP) * (innings_per_game)

`innings_per_game` varies by league (9 for MLB, 6-7 for most youth). Earned
runs exclude runs that scored due to errors.

Requires: earned/unearned run classification per runner scored.

### WHIP - Walks + Hits per IP
    WHIP = (BB + H) / IP

### K/9 - Strikeouts per 9 Innings
    K/9 = (K / IP) * 9

### BB/9 - Walks per 9 Innings
    BB/9 = (BB / IP) * 9

### H/9 - Hits per 9 Innings
    H/9 = (H / IP) * 9

### HR/9 - Home Runs per 9 Innings
    HR/9 = (HR / IP) * 9

### K/BB - Strikeout-to-Walk Ratio
    K/BB = K / BB

### BF - Batters Faced
Total plate appearances against this pitcher.

### Pitch Count
Total pitches thrown.

### WP - Wild Pitches
Pitches past the catcher ruled the pitcher's fault, allowing runners to
advance.

---

## Pitching - Sabermetric

### FIP - Fielding Independent Pitching
Estimates what a pitcher's ERA should be based only on outcomes the pitcher
controls (strikeouts, walks, HBP, home runs). Critical for youth baseball where
defense is highly variable.

    FIP = ((13*HR) + (3*(BB+HBP)) - (2*K)) / IP + FIP_constant

FIP_constant is calibrated so league FIP = league ERA. For youth, derive from
accumulated game data. MLB constant is typically ~3.10-3.20.

### xFIP - Expected FIP
FIP but replaces actual HR with expected HR using league-average HR/FB rate.
Removes HR luck.

    xFIP = ((13*(FB*lgHR/FB)) + (3*(BB+HBP)) - (2*K)) / IP + FIP_constant

Requires: fly balls allowed per pitcher, league HR/FB rate.

### K% - Strikeout Rate (Pitching)
    K% = K / BF

### BB% - Walk Rate (Pitching)
    BB% = BB / BF

### K-BB% - Strikeout Minus Walk Rate
The single most predictive pitching metric. Measures ability to strike batters
out without giving up free passes.

    K-BB% = K% - BB%

Elite: 20%+. Above average: 12-20%. Below average: <8%.

### BABIP Against
How often balls in play against this pitcher fall for hits.

    BABIP = (H - HR) / (BIP - HR)

High BABIP against suggests bad luck or poor defense behind the pitcher.

### LOB% - Left On Base Percentage
Percentage of runners the pitcher strands (does not allow to score).

    LOB% = (H + BB + HBP - R) / (H + BB + HBP - 1.4*HR)

League average ~72%. Extreme values (high or low) tend to regress.

### HR/FB - Home Run to Fly Ball Rate (Pitching)
    HR/FB = HR / FB

League average ~10-13%. Extreme values suggest HR luck.

Requires: fly balls allowed per pitcher.

### GB%, FB%, LD% - Batted Ball Distribution (Pitching)
Same formulas as batting, from the pitcher's perspective.

Requires: batted ball type per pitcher.

### SIERA - Skill-Interactive ERA
The most predictive ERA estimator. Unlike FIP, incorporates batted ball types
and their interactions with K and BB rates.

    SIERA = a - b*(K%) + c*(BB%) + d*(GB%) - e*(K%*GB%) + ... + constant

Uses ~10 regression coefficients. More predictive than FIP or xFIP because
ground-ball pitchers suppress hits even on balls in play.

Requires: K%, BB%, GB% per pitcher.

### Game Score (Bill James)
Single-number rating for a pitching start.

    Start with 50.
    +1 per out recorded
    +2 per IP after the 4th inning
    +1 per strikeout
    -2 per hit allowed
    -4 per earned run
    -2 per unearned run
    -1 per walk

### Pitches Per Inning
    P/IP = pitches / IP

Efficiency metric. Lower is better. Youth benchmark: <15 is efficient.

---

## Pitching - Plate Discipline

### SwStr% - Swinging Strike Rate
    SwStr% = swinging_strikes / total_pitches

Measures deception and movement. MLB average ~9.5%.

### CSW% - Called Strike + Whiff Rate
    CSW% = (called_strikes + swinging_strikes) / total_pitches

Created by Nick Pollack (2018). The best single-stat predictor of strikeout
ability. MLB average ~27-28%. Elite: 30%+.

### CStr% - Called Strike Rate
    CStr% = called_strikes / total_pitches

### FPS% - First Pitch Strike Percentage
    FPS% = first_pitch_strikes / PA

A first pitch strike is any pitch where the count is 0-0 and the result is
strike_swinging, strike_looking, foul, or ball_in_play. GameChanger's #1
recommended stat for pitcher development. Target: 60%+.

### Foul%
    Foul% = fouls / total_pitches

---

## Baserunning

### SB - Stolen Bases
Successful stolen base attempts.

### CS - Caught Stealing
Failed stolen base attempts.

### SB% - Stolen Base Success Rate
    SB% = SB / (SB + CS)

A steal attempt is +EV when SB% > ~72% (breakeven depends on run environment).

### R - Runs Scored
Times crossing home plate (same as batting R).

---

## Run Expectancy & Win Probability

### RE24 - Run Expectancy Based on 24 Base-Out States
The most powerful context-dependent metric. Measures how each play changed the
expected runs for the rest of the inning.

    RE24 = RE(end_state) - RE(start_state) + runs_scored_on_play

The run expectancy matrix has 24 cells: 8 base states (empty, 1st, 2nd, 3rd,
1st+2nd, 1st+3rd, 2nd+3rd, loaded) times 3 out states (0, 1, 2 outs). Each
cell holds the average runs expected from that state through the end of the
inning.

Example RE values (MLB averages, approximate):
- Bases empty, 0 outs: 0.48 runs
- Runner on 1st, 0 outs: 0.86 runs
- Bases loaded, 0 outs: 2.29 runs
- Bases loaded, 2 outs: 0.75 runs

Diamond-replay tracks complete base-out state on every event, making RE24
directly computable. For youth, calibrate the RE matrix from accumulated game
data.

### WPA - Win Probability Added
How much each play changed the batting team's probability of winning.

    WPA = WP(after_play) - WP(before_play)

Win probability depends on: score difference, inning, half (top/bottom),
base-out state, and home/away. Requires a win probability model calibrated to
the league's run environment and game length.

### LI - Leverage Index
The importance of a situation relative to an average situation. Average LI = 1.0.

- Low leverage: < 0.7
- Medium leverage: 0.7 - 1.5
- High leverage: > 1.5

Derived from the range of possible WP swings in a given game state.

---

## Composite & Emerging

### Comprehensive+
Composite batting metric (originated at Samford University, 2026). Combines 10
indicators into a single score using percentile normalization against a league
baseline.

Components: BB%, K%, ISO, wOBA, BABIP, GB%, LD%, HR/FB, P/PA, QAB%.

Each component is converted to a percentile rank. The composite is the average
percentile, scaled so 100 = league average.

Requires: league-wide baselines from accumulated game data.

### wPDI - Weighted Plate Discipline Index
Community-created pitching composite (FanGraphs fantasy community). Measures
control + deception + contact suppression in a single number.

Simplified version computable from our data:
    wPDI = w1*CStr% + w2*SwStr% + w3*(1-Foul%) + w4*(1-BB%) + w5*K%

Weights calibrated to run prevention. Full version requires zone data.

---

## Game-Level & Team Stats

### Linescore
Runs per inning for each team.

### Final Score
Total runs for each team.

### Pythagorean Win %
Expected win percentage from runs scored and allowed.

    Pythag% = RS^2 / (RS^2 + RA^2)

Only meaningful across multiple games.

### Pace / Game Tempo
- Time between half-innings (transition gaps)
- Dead time per inning
- Total game duration (first to last timestamp)

---

## Stats We Cannot Compute

These require Statcast, TrackMan, Rapsodo, HitTrax, or bat sensors:

- **Exit Velocity** (EV) - ball speed off bat
- **Launch Angle** (LA) - vertical angle off bat
- **Barrel%** - optimal EV+LA combination
- **Hard Hit%** - BIP with EV >= 95 mph
- **xBA, xSLG, xwOBA** - expected stats from EV+LA
- **Sprint Speed** - player speed in ft/sec
- **Bat Speed / Swing Length** - bat tracking metrics (Statcast 2024+)
- **Stuff+ / Location+ / Pitching+** - pitch quality models from velocity, spin, movement
- **Spin Rate / Spin Axis** - pitch spin characteristics
- **Pitch Movement** (IVB, HB) - break relative to spinless trajectory
- **OAA** - Outs Above Average fielding metric
- **Catcher Framing / Pop Time** - receiving and throwing metrics
- **Zone-based discipline** (O-Swing%, Z-Swing%, O-Contact%, Z-Contact%) - requires pitch location classification
