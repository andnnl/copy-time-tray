use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct Gist {
    pub id: String,
    pub description: Option<String>,
    pub files: HashMap<String, GistFile>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GistFile {
    pub filename: Option<String>,
    pub content: Option<String>,
    pub raw_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GistCreate<'a> {
    pub description: Option<&'a str>,
    pub files: HashMap<String, GistFileCreate<'a>>,
    pub public: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GistFileCreate<'a> {
    pub content: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GistUpdate<'a> {
    pub description: Option<&'a str>,
    pub files: HashMap<String, GistFileUpdate<'a>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GistFileUpdate<'a> {
    pub content: Option<&'a str>,
    pub filename: Option<&'a str>,
}

impl<'a> GistFileCreate<'a> {
    pub fn new(content: Option<&'a str>) -> Self {
        Self { content }
    }
}

impl<'a> GistUpdate<'a> {
    pub fn new(description: Option<&'a str>) -> Self {
        Self {
            description,
            files: HashMap::new(),
        }
    }

    pub fn add_file(mut self, name: &'a str, content: &'a str) -> Self {
        self.files.insert(
            name.into(),
            GistFileUpdate {
                content: Some(content),
                filename: Some(name),
            },
        );
        self
    }
}
