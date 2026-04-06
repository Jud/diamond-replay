use serde::Deserialize;

// ---------------------------------------------------------------------------
// Raw API event (outer wrapper from the game-streams endpoint)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
pub struct RawApiEvent {
    pub id: String,
    pub stream_id: String,
    pub sequence_number: i64,
    pub event_data: String,
}

// ---------------------------------------------------------------------------
// Parsed event data (inner JSON)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum EventData {
    Transaction { code: String, events: Vec<SubEvent> },
    Single(SubEvent),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubEvent {
    #[serde(default)]
    pub code: String,
    pub created_at: Option<i64>,
    #[serde(default)]
    pub attributes: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Typed enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PitchResult {
    Ball,
    StrikeSwinging,
    StrikeLooking,
    Foul,
    BallInPlay,
    HitByPitch,
    Unknown,
}

impl PitchResult {
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "ball" => Self::Ball,
            "strike_swinging" => Self::StrikeSwinging,
            "strike_looking" => Self::StrikeLooking,
            "foul" => Self::Foul,
            "ball_in_play" => Self::BallInPlay,
            "hit_by_pitch" => Self::HitByPitch,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayResult {
    BatterOut,
    BatterOutAdvanceRunners,
    InfieldFly,
    DroppedThirdStrikeBatterOut,
    SacrificeFly,
    SacrificeBunt,
    GroundOut,
    FlyOut,
    LineOut,
    PopOut,
    DoublPlay,
    Single,
    Double,
    Triple,
    HomeRun,
    FieldersChoice,
    Error,
    DroppedThirdStrike,
    Unknown,
}

impl PlayResult {
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "batter_out" => Self::BatterOut,
            "batter_out_advance_runners" => Self::BatterOutAdvanceRunners,
            "infield_fly" => Self::InfieldFly,
            "dropped_third_strike_batter_out" => Self::DroppedThirdStrikeBatterOut,
            "sacrifice_fly" => Self::SacrificeFly,
            "sacrifice_bunt" => Self::SacrificeBunt,
            "ground_out" => Self::GroundOut,
            "fly_out" => Self::FlyOut,
            "line_out" => Self::LineOut,
            "pop_out" => Self::PopOut,
            "double_play" => Self::DoublPlay,
            "single" => Self::Single,
            "double" => Self::Double,
            "triple" => Self::Triple,
            "home_run" => Self::HomeRun,
            "fielders_choice" => Self::FieldersChoice,
            "error" => Self::Error,
            "dropped_third_strike" => Self::DroppedThirdStrike,
            _ => Self::Unknown,
        }
    }

    #[must_use]
    pub fn is_batter_out(self) -> bool {
        matches!(
            self,
            Self::BatterOut
                | Self::BatterOutAdvanceRunners
                | Self::InfieldFly
                | Self::DroppedThirdStrikeBatterOut
                | Self::SacrificeFly
                | Self::SacrificeBunt
                | Self::GroundOut
                | Self::FlyOut
                | Self::LineOut
                | Self::PopOut
                | Self::DoublPlay
        )
    }

    #[must_use]
    pub fn is_advance_runners_out(self) -> bool {
        matches!(
            self,
            Self::BatterOutAdvanceRunners | Self::SacrificeFly | Self::SacrificeBunt
        )
    }

    #[must_use]
    pub fn is_dropped_third_strike(self) -> bool {
        matches!(
            self,
            Self::DroppedThirdStrike | Self::DroppedThirdStrikeBatterOut
        )
    }

    #[must_use]
    pub fn is_hit(self) -> bool {
        matches!(
            self,
            Self::Single | Self::Double | Self::Triple | Self::HomeRun
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrPlayType {
    StoleBase,
    PassedBall,
    WildPitch,
    CaughtStealing,
    OutOnLastPlay,
    PickedOff,
    RemainedOnLastPlay,
    AdvancedOnLastPlay,
    AdvancedOnError,
    OnSameError,
    OnSamePitch,
    DefensiveIndifference,
    AttemptedPickoff,
    OtherAdvance,
    OtherOut,
    Unknown,
}

impl BrPlayType {
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "stole_base" => Self::StoleBase,
            "passed_ball" => Self::PassedBall,
            "wild_pitch" => Self::WildPitch,
            "caught_stealing" => Self::CaughtStealing,
            "out_on_last_play" => Self::OutOnLastPlay,
            "picked_off" => Self::PickedOff,
            "remained_on_last_play" => Self::RemainedOnLastPlay,
            "advanced_on_last_play" => Self::AdvancedOnLastPlay,
            "advanced_on_error" => Self::AdvancedOnError,
            "on_same_error" => Self::OnSameError,
            "on_same_pitch" => Self::OnSamePitch,
            "defensive_indifference" => Self::DefensiveIndifference,
            "attempted_pickoff" => Self::AttemptedPickoff,
            "other_advance" => Self::OtherAdvance,
            "other_out" => Self::OtherOut,
            _ => Self::Unknown,
        }
    }

    #[must_use]
    pub fn is_out(self) -> bool {
        matches!(
            self,
            Self::CaughtStealing | Self::OutOnLastPlay | Self::PickedOff | Self::OtherOut
        )
    }

    #[must_use]
    pub fn is_chaos(self) -> bool {
        matches!(self, Self::StoleBase | Self::WildPitch | Self::PassedBall)
    }
}

/// The cause field on dropped-third-strike `ball_in_play` events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BipCause {
    WildPitch,
    PassedBall,
    Other,
}

impl BipCause {
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "wild_pitch" => Self::WildPitch,
            "passed_ball" => Self::PassedBall,
            _ => Self::Other,
        }
    }

    #[must_use]
    pub fn is_ball_away(self) -> bool {
        matches!(self, Self::WildPitch | Self::PassedBall)
    }
}

/// The `playType` on `ball_in_play` events (batted ball type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BipPlayType {
    GroundBall,
    HardGroundBall,
    FlyBall,
    LineDrive,
    PopFly,
    Other,
}

impl BipPlayType {
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "ground_ball" => Self::GroundBall,
            "hard_ground_ball" => Self::HardGroundBall,
            "fly_ball" => Self::FlyBall,
            "line_drive" => Self::LineDrive,
            "pop_fly" => Self::PopFly,
            _ => Self::Other,
        }
    }
}

// ---------------------------------------------------------------------------
// Attribute helpers — extract typed fields from serde_json::Value
// ---------------------------------------------------------------------------

#[must_use]
pub fn attr_str<'a>(attrs: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    attrs.get(key)?.as_str()
}

#[must_use]
pub fn attr_bool(attrs: &serde_json::Value, key: &str, default: bool) -> bool {
    attrs
        .get(key)
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(default)
}

/// Extract a non-negative integer attribute as `usize`.
#[must_use]
pub fn attr_usize(attrs: &serde_json::Value, key: &str) -> Option<usize> {
    let v = attrs.get(key)?.as_i64()?;
    usize::try_from(v).ok()
}

/// Extract an integer attribute as `i32`.
#[must_use]
pub fn attr_i32(attrs: &serde_json::Value, key: &str) -> Option<i32> {
    let v = attrs.get(key)?.as_i64()?;
    i32::try_from(v).ok()
}
