//! Structured user elicitation and interactive clarification protocol.
//!
//! Provides the protocol schema, validation, and XML envelope encoding when an agent
//! requires user clarification (single-choice, multi-choice, boolean confirmation,
//! or freeform text) before proceeding with autonomous execution.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use uuid::Uuid;

/// Elicitation question kind and choices.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElicitationKind {
    /// Single choice among a provided list of options.
    SingleChoice {
        options: Vec<String>,
        default_index: Option<usize>,
    },
    /// Multiple choices allowed among provided options.
    MultiChoice {
        options: Vec<String>,
        default_indices: Vec<usize>,
    },
    /// Direct Yes/No confirmation.
    Confirmation { default_yes: bool },
    /// Freeform text input.
    FreeformText {
        placeholder: Option<String>,
        max_chars: Option<usize>,
    },
}

/// Request emitted by an agent to solicit user clarification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElicitationRequest {
    pub id: String,
    pub prompt: String,
    pub kind: ElicitationKind,
    pub timeout_secs: Option<u64>,
    pub created_at: i64,
}

impl ElicitationRequest {
    pub fn new(prompt: impl Into<String>, kind: ElicitationKind) -> Self {
        Self {
            id: format!("elicit-{}", Uuid::new_v4().simple()),
            prompt: prompt.into(),
            kind,
            timeout_secs: Some(300), // 5 minute default
            created_at: Utc::now().timestamp_millis(),
        }
    }

    /// Renders the elicitation into an inline XML envelope for LLM or terminal consumption.
    pub fn to_xml_envelope(&self) -> String {
        let mut out = format!(
            "<elicitation id=\"{}\">\n  <prompt>{}</prompt>\n",
            self.id, self.prompt
        );
        match &self.kind {
            ElicitationKind::SingleChoice {
                options,
                default_index,
            } => {
                out.push_str("  <type>single_choice</type>\n  <options>\n");
                for (i, opt) in options.iter().enumerate() {
                    let is_def = default_index.map(|d| d == i).unwrap_or(false);
                    out.push_str(&format!(
                        "    <option index=\"{}\" default=\"{}\">{}</option>\n",
                        i, is_def, opt
                    ));
                }
                out.push_str("  </options>\n");
            }
            ElicitationKind::MultiChoice {
                options,
                default_indices,
            } => {
                out.push_str("  <type>multi_choice</type>\n  <options>\n");
                for (i, opt) in options.iter().enumerate() {
                    let is_def = default_indices.contains(&i);
                    out.push_str(&format!(
                        "    <option index=\"{}\" default=\"{}\">{}</option>\n",
                        i, is_def, opt
                    ));
                }
                out.push_str("  </options>\n");
            }
            ElicitationKind::Confirmation { default_yes } => {
                out.push_str(&format!(
                    "  <type>confirmation</type>\n  <default_yes>{}</default_yes>\n",
                    default_yes
                ));
            }
            ElicitationKind::FreeformText {
                placeholder,
                max_chars,
            } => {
                out.push_str("  <type>freeform</type>\n");
                if let Some(p) = placeholder {
                    out.push_str(&format!("  <placeholder>{}</placeholder>\n", p));
                }
                if let Some(mc) = max_chars {
                    out.push_str(&format!("  <max_chars>{}</max_chars>\n", mc));
                }
            }
        }
        out.push_str("</elicitation>");
        out
    }
}

/// User's response payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElicitationPayload {
    SelectedOption(String),
    SelectedOptions(Vec<String>),
    Confirmed(bool),
    Text(String),
    TimedOut,
    Dismissed,
}

/// Completed elicitation response with timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElicitationResponse {
    pub request_id: String,
    pub payload: ElicitationPayload,
    pub answered_at: i64,
}

impl ElicitationResponse {
    pub fn new(request_id: impl Into<String>, payload: ElicitationPayload) -> Self {
        Self {
            request_id: request_id.into(),
            payload,
            answered_at: Utc::now().timestamp_millis(),
        }
    }

    /// Renders the answer into a `<elicitation_reply>` XML envelope for LLM context resumption.
    pub fn to_xml_reply(&self) -> String {
        let body = match &self.payload {
            ElicitationPayload::SelectedOption(opt) => format!("<selected>{}</selected>", opt),
            ElicitationPayload::SelectedOptions(opts) => {
                let items: Vec<String> =
                    opts.iter().map(|o| format!("<item>{}</item>", o)).collect();
                format!("<selected_items>{}</selected_items>", items.join(""))
            }
            ElicitationPayload::Confirmed(c) => format!("<confirmed>{}</confirmed>", c),
            ElicitationPayload::Text(t) => format!("<text>{}</text>", t),
            ElicitationPayload::TimedOut => "<status>timed_out</status>".to_string(),
            ElicitationPayload::Dismissed => "<status>dismissed</status>".to_string(),
        };
        format!(
            "<elicitation_reply id=\"{}\">{}</elicitation_reply>",
            self.request_id, body
        )
    }
}

/// Validation error for user response against request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElicitationValidationError {
    RequestIdMismatch { expected: String, found: String },
    InvalidOptionSelection { option: String },
    PayloadKindMismatch { expected: &'static str },
    TextExceedsMaxChars { max: usize, actual: usize },
}

impl fmt::Display for ElicitationValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestIdMismatch { expected, found } => {
                write!(
                    f,
                    "Request ID mismatch: expected {}, found {}",
                    expected, found
                )
            }
            Self::InvalidOptionSelection { option } => {
                write!(f, "Selected option '{}' is not among valid choices", option)
            }
            Self::PayloadKindMismatch { expected } => {
                write!(f, "Payload kind mismatch: expected {}", expected)
            }
            Self::TextExceedsMaxChars { max, actual } => {
                write!(
                    f,
                    "Text length {} exceeds maximum allowed {} characters",
                    actual, max
                )
            }
        }
    }
}

impl Error for ElicitationValidationError {}

/// Validates an elicitation response against its original request schema.
pub fn validate_elicitation_response(
    req: &ElicitationRequest,
    resp: &ElicitationResponse,
) -> Result<(), ElicitationValidationError> {
    if req.id != resp.request_id {
        return Err(ElicitationValidationError::RequestIdMismatch {
            expected: req.id.clone(),
            found: resp.request_id.clone(),
        });
    }

    match (&req.kind, &resp.payload) {
        (_, ElicitationPayload::TimedOut) | (_, ElicitationPayload::Dismissed) => Ok(()),
        (
            ElicitationKind::SingleChoice { options, .. },
            ElicitationPayload::SelectedOption(opt),
        ) => {
            if options.contains(opt) {
                Ok(())
            } else {
                Err(ElicitationValidationError::InvalidOptionSelection {
                    option: opt.clone(),
                })
            }
        }
        (
            ElicitationKind::MultiChoice { options, .. },
            ElicitationPayload::SelectedOptions(opts),
        ) => {
            for opt in opts {
                if !options.contains(opt) {
                    return Err(ElicitationValidationError::InvalidOptionSelection {
                        option: opt.clone(),
                    });
                }
            }
            Ok(())
        }
        (ElicitationKind::Confirmation { .. }, ElicitationPayload::Confirmed(_)) => Ok(()),
        (ElicitationKind::FreeformText { max_chars, .. }, ElicitationPayload::Text(text)) => {
            if let Some(max) = max_chars
                && text.chars().count() > *max
            {
                return Err(ElicitationValidationError::TextExceedsMaxChars {
                    max: *max,
                    actual: text.chars().count(),
                });
            }
            Ok(())
        }
        (ElicitationKind::SingleChoice { .. }, _) => {
            Err(ElicitationValidationError::PayloadKindMismatch {
                expected: "SelectedOption",
            })
        }
        (ElicitationKind::MultiChoice { .. }, _) => {
            Err(ElicitationValidationError::PayloadKindMismatch {
                expected: "SelectedOptions",
            })
        }
        (ElicitationKind::Confirmation { .. }, _) => {
            Err(ElicitationValidationError::PayloadKindMismatch {
                expected: "Confirmed",
            })
        }
        (ElicitationKind::FreeformText { .. }, _) => {
            Err(ElicitationValidationError::PayloadKindMismatch { expected: "Text" })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_choice_xml_and_validation() {
        let req = ElicitationRequest::new(
            "Which database backend would you prefer?",
            ElicitationKind::SingleChoice {
                options: vec![
                    "SQLite".to_string(),
                    "PostgreSQL".to_string(),
                    "DuckDB".to_string(),
                ],
                default_index: Some(0),
            },
        );

        let xml = req.to_xml_envelope();
        assert!(xml.contains("<type>single_choice</type>"));
        assert!(xml.contains("<option index=\"0\" default=\"true\">SQLite</option>"));

        // Valid response
        let resp_valid = ElicitationResponse::new(
            &req.id,
            ElicitationPayload::SelectedOption("PostgreSQL".to_string()),
        );
        assert!(validate_elicitation_response(&req, &resp_valid).is_ok());
        assert!(
            resp_valid
                .to_xml_reply()
                .contains("<selected>PostgreSQL</selected>")
        );

        // Invalid response
        let resp_invalid = ElicitationResponse::new(
            &req.id,
            ElicitationPayload::SelectedOption("MySQL".to_string()),
        );
        assert!(matches!(
            validate_elicitation_response(&req, &resp_invalid),
            Err(ElicitationValidationError::InvalidOptionSelection { .. })
        ));
    }

    #[test]
    fn test_confirmation_and_freeform() {
        let confirm_req = ElicitationRequest::new(
            "Do you want to apply migrations?",
            ElicitationKind::Confirmation { default_yes: true },
        );
        let resp_yes =
            ElicitationResponse::new(&confirm_req.id, ElicitationPayload::Confirmed(true));
        assert!(validate_elicitation_response(&confirm_req, &resp_yes).is_ok());
        assert_eq!(
            resp_yes.to_xml_reply(),
            format!(
                "<elicitation_reply id=\"{}\"><confirmed>true</confirmed></elicitation_reply>",
                confirm_req.id
            )
        );

        let freeform_req = ElicitationRequest::new(
            "Provide service token",
            ElicitationKind::FreeformText {
                placeholder: Some("token-...".to_string()),
                max_chars: Some(10),
            },
        );
        let resp_toolong = ElicitationResponse::new(
            &freeform_req.id,
            ElicitationPayload::Text("this-is-too-long-text".to_string()),
        );
        assert!(matches!(
            validate_elicitation_response(&freeform_req, &resp_toolong),
            Err(ElicitationValidationError::TextExceedsMaxChars {
                max: 10,
                actual: 21
            })
        ));
    }
}
