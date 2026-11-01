//! Production synchronous guardian reviewer output schema and tolerant JSON parser.
//! Parses and validates risk assessment responses from automated security reviewers.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Guardian assessment outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuardianAssessmentOutcome {
    Allow,
    Deny,
}

/// Guardian risk level classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuardianRiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// User authorization level for reviewed action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GuardianUserAuthorization {
    Unknown,
    Low,
    Medium,
    High,
}

/// Structured output contract that the guardian reviewer must satisfy.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct StructuredGuardianAssessment {
    pub risk_level: GuardianRiskLevel,
    pub user_authorization: GuardianUserAuthorization,
    pub outcome: GuardianAssessmentOutcome,
    pub rationale: String,
}

impl StructuredGuardianAssessment {
    pub fn is_approved(&self) -> bool {
        self.outcome == GuardianAssessmentOutcome::Allow
    }
}

#[derive(Deserialize)]
struct GuardianAssessmentPayload {
    risk_level: Option<GuardianRiskLevel>,
    user_authorization: Option<GuardianUserAuthorization>,
    outcome: GuardianAssessmentOutcome,
    rationale: Option<String>,
}

/// Tolerant parser for Guardian assessment JSON responses.
/// Accepts raw JSON or markdown-fenced / wrapped JSON.
pub fn parse_guardian_assessment(text: &str) -> anyhow::Result<StructuredGuardianAssessment> {
    let parsed_payload =
        if let Ok(payload) = serde_json::from_str::<GuardianAssessmentPayload>(text) {
            payload
        } else if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) {
            if start < end {
                if let Some(slice) = text.get(start..=end) {
                    serde_json::from_str::<GuardianAssessmentPayload>(slice)?
                } else {
                    anyhow::bail!("unable to extract JSON slice from text");
                }
            } else {
                anyhow::bail!("guardian assessment was not valid JSON");
            }
        } else {
            anyhow::bail!("guardian assessment was not valid JSON");
        };

    let outcome = parsed_payload.outcome;
    let risk_level = parsed_payload.risk_level.unwrap_or(match outcome {
        GuardianAssessmentOutcome::Allow => GuardianRiskLevel::Low,
        GuardianAssessmentOutcome::Deny => GuardianRiskLevel::High,
    });
    let rationale = parsed_payload
        .rationale
        .filter(|r| !r.trim().is_empty())
        .unwrap_or_else(|| match outcome {
            GuardianAssessmentOutcome::Allow => {
                "Auto-review returned a low-risk allow decision.".to_string()
            }
            GuardianAssessmentOutcome::Deny => {
                "Auto-review returned a deny decision without a rationale.".to_string()
            }
        });

    Ok(StructuredGuardianAssessment {
        risk_level,
        user_authorization: parsed_payload
            .user_authorization
            .unwrap_or(GuardianUserAuthorization::Unknown),
        outcome,
        rationale,
    })
}

/// JSON schema supplied as output constraint schema for guardian models.
pub fn guardian_output_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "risk_level": {
                "type": "string",
                "enum": ["low", "medium", "high", "critical"]
            },
            "user_authorization": {
                "type": "string",
                "enum": ["unknown", "low", "medium", "high"]
            },
            "outcome": {
                "type": "string",
                "enum": ["allow", "deny"]
            },
            "rationale": {
                "type": "string"
            }
        },
        "required": ["outcome"]
    })
}

/// Prompt fragment describing the guardian JSON response format.
pub fn guardian_output_contract_prompt() -> &'static str {
    r#"You may use read-only tool checks to gather any additional context you need before deciding. When you are ready to answer, your final message must be strict JSON.

For low-risk actions, give the final answer directly: {"outcome":"allow"}.

For anything else, use this JSON schema:
{
  "risk_level": "low" | "medium" | "high" | "critical",
  "user_authorization": "unknown" | "low" | "medium" | "high",
  "outcome": "allow" | "deny",
  "rationale": string
}"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_allow() {
        let text = r#"{"outcome": "allow"}"#;
        let res = parse_guardian_assessment(text).expect("must parse");
        assert_eq!(res.outcome, GuardianAssessmentOutcome::Allow);
        assert_eq!(res.risk_level, GuardianRiskLevel::Low);
        assert_eq!(res.user_authorization, GuardianUserAuthorization::Unknown);
        assert!(res.rationale.contains("low-risk allow decision"));
    }

    #[test]
    fn test_parse_wrapped_prose_json() {
        let text = "Here is my assessment of the command:\n```json\n{\"outcome\": \"deny\", \"risk_level\": \"critical\", \"rationale\": \"Destructive command\"}\n```\nThank you.";
        let res = parse_guardian_assessment(text).expect("must parse embedded JSON");
        assert_eq!(res.outcome, GuardianAssessmentOutcome::Deny);
        assert_eq!(res.risk_level, GuardianRiskLevel::Critical);
        assert_eq!(res.rationale, "Destructive command");
    }

    #[test]
    fn test_parse_invalid_fails() {
        let text = "Not a json at all";
        assert!(parse_guardian_assessment(text).is_err());
    }
}
