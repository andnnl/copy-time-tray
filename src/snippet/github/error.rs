use thiserror::Error;

#[derive(Debug, Error)]
pub enum GithubError {
    #[error("GitHub Gist was not found")]
    NotFound,
    #[error("GitHub authentication or authorization failed ({0})")]
    Auth(u16),
    #[error("GitHub HTTP error ({status}): {body}")]
    Http { status: u16, body: String },
    #[error("GitHub network error: {0}")]
    Network(String),
    #[error("GitHub JSON parse error: {0}")]
    Parse(String),
}

pub type Result<T> = std::result::Result<T, GithubError>;
