pub mod error;
pub mod client;
pub mod types;

pub use client::GiteeClient;
pub use error::{GiteeError, Result};
pub use types::{Gist, GistFile, GistCreate, GistFileCreate, GistUpdate};
