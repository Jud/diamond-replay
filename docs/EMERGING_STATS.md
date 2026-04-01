# Emerging & Community Baseball Analytics

Stats gaining traction in the baseball analytics community that aren't yet
mainstream. Includes metrics from influencers, fantasy analysts, college
research, and the Statcast frontier.

---

## Computable From Play-by-Play

### CSW% - Called Strike + Whiff Rate
Created by Nick Pollack (Pitcher List, 2018). Rapidly adopted by the analytics
community as the single best predictor of strikeout ability from pitch-level
data.

    CSW% = (called_strikes + swinging_strikes) / total_pitches

MLB average ~27-28%. Elite: 30%+. Stabilizes quickly (~200 pitches). Now
tracked by FanGraphs, Baseball Savant, and most fantasy platforms.

### QAB% - Quality At-Bat Rate
Promoted heavily by GameChanger for youth baseball. Measures process over
outcomes. Definitions vary; the most common:

A quality at-bat is any PA resulting in:
- A hit, walk, HBP, sac fly, sac bunt, or ROE
- 3+ pitches seen after reaching a 2-strike count
- 6+ total pitches in the at-bat
- A productive out (advances a runner)

Teams with 12+ QABs per game win 60%+ of the time (GameChanger data).

### Comprehensive+ (Samford University, 2026)
Composite batting metric designed for college baseball. Combines 10 indicators
using percentile normalization:

1. BB%
2. K%
3. ISO
4. wOBA
5. BABIP
6. GB%
7. LD%
8. HR/FB
9. P/PA (pitches per PA)
10. QAB%

Each component is ranked as a percentile against the league. Final score = mean
percentile, scaled to 100 = average. Designed to capture both outcomes and
approach in a single number.

Source: https://www.samford.edu/sports-analytics/fans/2026/What-Batting-Average-Is-Missing-And-How-We-Built-Something-Better

### wPDI - Weighted Plate Discipline Index
Created by the FanGraphs fantasy community. Pitching composite measuring
control + deception + contact suppression.

Full version requires zone data (O-Swing%, Z-Contact%, etc.). A simplified
version is computable from pitch outcomes:

    wPDI_simple = w1*CStr% + w2*SwStr% + w3*(1-BB%) + w4*K%

Source: https://fantasy.fangraphs.com/introducing-weighted-plate-discipline-index-wpdi-for-pitchers/

### K-BB% Tiers
Widely used on baseball Twitter/Instagram for quick pitcher evaluation. The
community has standardized tier labels:

- Elite: 20%+
- Great: 15-20%
- Above Average: 12-15%
- Average: 8-12%
- Below Average: 5-8%
- Poor: <5%

### Competitive At-Bat Rate
Emerging youth metric tracking whether a batter reached a 2-strike count.
Measures willingness to compete regardless of outcome. Gaining traction in
travel ball and high school analytics.

### Hard Hit Ball Rate (Youth Version)
GameChanger's #3 recommended stat. In their data, a "hard hit ball" is a
ground ball or line drive hit with authority (GameChanger classifies
`hard_ground_ball` as a distinct play type). HHB rate correlates with offensive
production more reliably than batting average at the youth level.

---

## Requires Hardware (Statcast/TrackMan/Sensors)

### Stuff+ / Location+ / Pitching+ (FanGraphs)
Model-based pitch quality metrics. Each pitch graded on a scale where 100 =
average.

- **Stuff+**: Predicts whiff ability from velocity, spin, movement, release
  point, extension. Ignores location. Stabilizes in ~80 pitches.
- **Location+**: Rates pitch placement given count, handedness, zone
  probability. Needs ~400 pitches to stabilize.
- **Pitching+**: Stuff + Location combined. Beats preseason projections after
  ~250 pitches (relievers) or ~400 (starters).

Now integrated into FanGraphs leaderboards.
Source: https://library.fangraphs.com/pitching/stuff-location-and-pitching-primer/

### PitchingBot (FanGraphs)
XGBoost ML model grading every pitch on a 20-80 scouting scale. Inputs:
handedness, zone height, count, velocity, spin rate, movement, release point,
extension, location. Produces per-pitch expected run value, stuff grade, command
grade.

Source: https://library.fangraphs.com/pitching/pitchingbot-pitch-modeling-primer/

### tjStuff+ (TJStats / Thomas Nestico)
Community-created pitch model by Thomas Nestico. Predicts run value per pitch,
grades on 20-80 scale. Popular on analytics Twitter. Uses velocity, spin rate,
movement.

Source: https://tjstats.ca/

### Bat Speed / Swing Length / Squared-Up Rate (Statcast 2024+)
MLB's newest tracking metrics from Hawk-Eye bat tracking:

- **Bat Speed**: Speed of the bat at the sweet spot (6 inches from head) in
  mph. MLB average ~72 mph.
- **Swing Length**: Total distance the bat head travels in the swing. Shorter
  swings correlate with better contact. MLB average ~7.3 ft.
- **Squared-Up Rate**: Actual EV as a percentage of theoretical max EV (based
  on bat speed + pitch speed). Measures quality of contact.
- **Blasts**: Squared-up + bat speed >= 75 mph. The ultimate quality-contact
  event.
- **Swords**: Swings where squared-up rate is extremely low (whiff or weak
  contact with a slow swing).

Source: https://www.mlb.com/news/new-statcast-swing-metrics-2025

### Swing Path / Attack Angle / Attack Direction (Statcast 2025+)
The newest Statcast metrics:

- **Swing Path**: Bat angle during the last 40ms before contact. MLB average
  ~32 degrees. Range 20-50.
- **Attack Angle**: Vertical direction of the sweet spot at contact. MLB
  average ~10 degrees.
- **Attack Direction**: Horizontal angle at contact. Positive = pull side.

### Seam-Shifted Wake (SSW)
Non-Magnus movement caused by baseball seam orientation. An active research
frontier in pitch design. Measures deviation between expected movement (from
spin) and actual movement.

Source: https://www.drivelinebaseball.com/2021/03/the-impact-of-seam-shifted-wakes-on-pitch-quality/

---

## Competitive Landscape

### ScoutBall AI (scoutballai.com)
Launched July 2025, provisional patent secured. Ingests GameChanger stats and
layers AI-powered analysis: lineup optimization, pitch charting with zone
analysis, personalized training plans. Targets youth and amateur baseball.

### GameChanger Analytics
Tracks 150+ stats for 500,000+ teams. Promotes three stats for youth
development:
1. First Pitch Strike % (FPS%)
2. Hard Hit Balls (HHB)
3. Quality At-Bats (QAB)

### ABS Challenge System (MLB 2026)
Automated Ball-Strike system debuted Opening Day 2026 (T-Mobile sponsored).
Generating new stat categories around challenge decisions. Not relevant for
youth yet but will likely trickle down.

---

## References

- FanGraphs Library: https://library.fangraphs.com/
- Baseball Savant: https://baseballsavant.mlb.com/
- Pitcher List (CSW%): https://pitcherlist.com/csw-rate-an-intro-to-an-important-new-metric/
- GameChanger Analytics Report: https://www.prnewswire.com/news-releases/gamechanger-baseball-and-softball-analytics-report-the-3-stats-every-player-coach-should-utilize-to-improve-performance-300218570.html
- Samford Comprehensive+: https://www.samford.edu/sports-analytics/fans/2026/What-Batting-Average-Is-Missing-And-How-We-Built-Something-Better
- TJStats: https://tjstats.ca/
- ScoutBall AI: https://www.scoutballai.com/
- Cronkite/ASU on Sabermetrics 2025: https://cronkitenews.azpbs.org/2025/05/09/mlb-sabermetrics-analytics-redefining-baseball-2025/
