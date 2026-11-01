//! Candidates catalog for fuzzy searching and `@` mentions autocomplete.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CandidateType {
    File,
    Directory,
    Skill,
    Tool,
    GitCommit,
    Symbol,
}

impl CandidateType {
    pub fn badge(&self) -> &'static str {
        match self {
            Self::File => "FILE",
            Self::Directory => "DIR",
            Self::Skill => "SKILL",
            Self::Tool => "TOOL",
            Self::GitCommit => "GIT",
            Self::Symbol => "SYM",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub display_name: String,
    pub description: Option<String>,
    pub candidate_type: CandidateType,
    pub insert_text: String,
    pub score: i64,
    pub match_indices: Vec<usize>,
}

impl Candidate {
    pub fn new(
        display: impl Into<String>,
        ctype: CandidateType,
        insert: impl Into<String>,
    ) -> Self {
        Self {
            display_name: display.into(),
            description: None,
            candidate_type: ctype,
            insert_text: insert.into(),
            score: 0,
            match_indices: Vec::new(),
        }
    }

    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}
