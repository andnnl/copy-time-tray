use thiserror::Error;

#[derive(Debug, Error)]
pub enum GiteeError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Rate limited (429), retry after {0}s")]
    RateLimited(u64),

    #[error("Authentication failed ({0}): check your token")]
    Auth(u16),
}

pub type Result<T> = std::result::Result<T, GiteeError>;
