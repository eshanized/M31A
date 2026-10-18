//! Shared decoding for SQLite rows representing lifecycle aggregates.

use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use sqlx::{Row, sqlite::SqliteRow};
use std::str::FromStr;

use crate::error::M31AError;
use crate::state::intake::AutonomyMode;
use crate::state::policy_context::PolicyContext;
use crate::state_machine::ExecutionRunState;

/// Shared JSON payload fields persisted by Mission and ExecutionRun rows.
pub(crate) struct EncodedAggregateFields {
    pub(crate) policy_context: String,
    pub(crate) constraints: String,
    pub(crate) requirements: String,
    pub(crate) success_criteria: String,
}

/// Serialize fields common to both lifecycle aggregate schemas.
pub(crate) fn encode_aggregate_fields(
    policy_context: &PolicyContext,
    constraints: &[String],
    requirements: &[String],
    success_criteria: &[String],
) -> Result<EncodedAggregateFields, M31AError> {
    Ok(EncodedAggregateFields {
        policy_context: serde_json::to_string(policy_context)
            .map_err(|error| M31AError::validation(error.to_string()))?,
        constraints: serde_json::to_string(constraints)
            .map_err(|error| M31AError::validation(error.to_string()))?,
        requirements: serde_json::to_string(requirements)
            .map_err(|error| M31AError::validation(error.to_string()))?,
        success_criteria: serde_json::to_string(success_criteria)
            .map_err(|error| M31AError::validation(error.to_string()))?,
    })
}

/// Decode a JSON-backed column, failing closed on corrupt persisted data.
pub(crate) fn decode_json_column<T: DeserializeOwned>(
    row: &SqliteRow,
    column: &str,
    aggregate: &str,
) -> Result<T, M31AError> {
    let raw: String = row.try_get(column)?;
    serde_json::from_str(&raw).map_err(|error| {
        M31AError::persistence(format!("corrupt {aggregate} {column} JSON: {error}"))
    })
}

/// Decode an autonomy mode without silently granting a default mode.
pub(crate) fn decode_autonomy_mode(
    row: &SqliteRow,
    aggregate: &str,
) -> Result<AutonomyMode, M31AError> {
    let raw: String = row.try_get("mode")?;
    AutonomyMode::from_str(&raw).map_err(|error| {
        M31AError::persistence(format!(
            "corrupt {aggregate} autonomy mode '{raw}': {error}"
        ))
    })
}

/// Decode a lifecycle state using the canonical state vocabulary.
pub(crate) fn decode_aggregate_status(
    row: &SqliteRow,
    aggregate: &str,
) -> Result<ExecutionRunState, M31AError> {
    let raw: String = row.try_get("status")?;
    ExecutionRunState::from_str(&raw).map_err(|error| {
        M31AError::persistence(format!("corrupt {aggregate} status '{raw}': {error}"))
    })
}

/// Decode a required RFC 3339 timestamp.
pub(crate) fn decode_required_timestamp(
    row: &SqliteRow,
    column: &str,
    aggregate: &str,
) -> Result<DateTime<Utc>, M31AError> {
    let raw: String = row.try_get(column)?;
    parse_timestamp(&raw, column, aggregate)
}

/// Decode an optional RFC 3339 timestamp, rejecting malformed non-null values.
pub(crate) fn decode_optional_timestamp(
    row: &SqliteRow,
    column: &str,
    aggregate: &str,
) -> Result<Option<DateTime<Utc>>, M31AError> {
    let raw: Option<String> = row.try_get(column)?;
    raw.map(|value| parse_timestamp(&value, column, aggregate))
        .transpose()
}

fn parse_timestamp(raw: &str, column: &str, aggregate: &str) -> Result<DateTime<Utc>, M31AError> {
    DateTime::parse_from_rfc3339(raw)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|error| {
            M31AError::persistence(format!(
                "corrupt {aggregate} {column} timestamp '{raw}': {error}"
            ))
        })
}
