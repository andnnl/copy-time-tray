use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct Gist {
    pub id: String,
    pub description: Option<String>,
    pub files: HashMap<String, GistFile>,
    pub public: bool,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GistFile {
    pub content: Option<String>,
    #[serde(default)]
    pub truncated: bool,
    pub raw_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GistCreate<'a> {
    pub description: &'a str,
    pub files: HashMap<String, GistFileCreate<'a>>,
    pub public: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GistUpdate<'a> {
    pub files: HashMap<String, GistFileCreate<'a>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GistFileCreate<'a> {
    pub content: &'a str,
}

impl<'a> GistCreate<'a> {
    pub fn private_tree(description: &'a str, content: &'a str) -> Self {
        Self {
            description,
            files: HashMap::from([(String::from("tree.json"), GistFileCreate::new(content))]),
            public: false,
        }
    }
}

impl<'a> GistUpdate<'a> {
    pub fn tree(content: &'a str) -> Self {
        Self {
            files: HashMap::from([(String::from("tree.json"), GistFileCreate::new(content))]),
        }
    }
}

impl<'a> GistFileCreate<'a> {
    pub fn new(content: &'a str) -> Self {
        Self { content }
    }
}
