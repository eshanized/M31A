//! Shared decoding for SQLite rows representing lifecycle aggregates.

use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use sqlx::{Row, sqlite::SqliteRow};
use std::str::FromStr;

use crate::error::M31AError;
use crate::state::intake::AutonomyMode;
use crate::state_machine::ExecutionRunState;

/// Decode a JSON-backed column, failing closed on corrupt persisted data.
pub(crate) fn decode_json_column<T: DeserializeOwned>(
    row: &SqliteRow,
    column: &str,
    aggregate: &str,
) -> Result<T, M31AError> {
    let raw: String = row.try_get(column)?;
    serde_json::from_str(&raw).map_err(|error| {
        M31AError::persistence(format!(
            "corrupt {aggregate} {column} JSON: {error}"
        ))
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
        M31AError::persistence(format!(
            "corrupt {aggregate} status '{raw}': {error}"
        ))
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

fn parse_timestamp(
    raw: &str,
    column: &str,
    aggregate: &str,
) -> Result<DateTime<Utc>, M31AError> {
    DateTime::parse_from_rfc3339(raw)
        .map(|timestamp| timestamp.with_timezone(&Utc))
        .map_err(|error| {
            M31AError::persistence(format!(
                "corrupt {aggregate} {column} timestamp '{raw}': {error}"
            ))
        })
}
