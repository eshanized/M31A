//! Core AST symbol representation and source ranges.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AstSymbolKind {
    Function,
    Method,
    Struct,
    Enum,
    Class,
    Interface,
    Trait,
    Impl,
    TypeAlias,
    Constant,
    Module,
    Property,
}

impl AstSymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Class => "class",
            Self::Interface => "interface",
            Self::Trait => "trait",
            Self::Impl => "impl",
            Self::TypeAlias => "type",
            Self::Constant => "const",
            Self::Module => "module",
            Self::Property => "property",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRange {
    pub start_line: usize,
    pub end_line: usize,
    pub start_col: usize,
    pub end_col: usize,
}

impl SourceRange {
    pub fn new(start_line: usize, end_line: usize, start_col: usize, end_col: usize) -> Self {
        Self {
            start_line,
            end_line,
            start_col,
            end_col,
        }
    }

    pub fn lines(&self) -> usize {
        if self.end_line >= self.start_line {
            self.end_line - self.start_line + 1
        } else {
            1
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AstSymbol {
    pub name: String,
    pub kind: AstSymbolKind,
    pub file_path: PathBuf,
    pub range: SourceRange,
    pub signature: String,
    pub doc_comment: Option<String>,
    pub parent_scope: Option<String>,
    pub is_public: bool,
    pub children: Vec<AstSymbol>,
}

impl AstSymbol {
    pub fn new(
        name: impl Into<String>,
        kind: AstSymbolKind,
        file_path: PathBuf,
        range: SourceRange,
        signature: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            file_path,
            range,
            signature: signature.into(),
            doc_comment: None,
            parent_scope: None,
            is_public: true,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticChunk {
    pub symbol_name: String,
    pub kind: AstSymbolKind,
    pub range: SourceRange,
    pub code: String,
    pub token_estimate: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallReference {
    pub caller_name: String,
    pub callee_name: String,
    pub file_path: PathBuf,
    pub line: usize,
}
