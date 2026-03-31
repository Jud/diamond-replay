use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReplayError {
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("no events after undo resolution")]
    NoEvents,

    #[error("missing set_teams event")]
    MissingTeams,

    #[error("invalid event data: {0}")]
    InvalidEvent(String),
}

pub type Result<T> = std::result::Result<T, ReplayError>;
