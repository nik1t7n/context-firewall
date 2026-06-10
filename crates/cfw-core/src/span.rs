use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    ReplacedToolResult,
    AdvisoryWrapper,
    ObservedOnly,
    Blocked,
    Unknown,
}

impl DeliveryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReplacedToolResult => "replaced_tool_result",
            Self::AdvisoryWrapper => "advisory_wrapper",
            Self::ObservedOnly => "observed_only",
            Self::Blocked => "blocked",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanRecord {
    pub id: String,
    pub session_id: String,
    pub kind: String,
    pub source: String,
    pub command: Option<String>,
    pub cwd: Option<String>,
    pub exit_code: Option<i32>,
    pub raw_bytes: i64,
    pub raw_estimated_tokens: i64,
    pub returned_bytes: i64,
    pub returned_estimated_tokens: i64,
    pub hash: String,
    pub reducer: Option<String>,
    pub policy_action: String,
    pub delivery_status: DeliveryStatus,
    pub delivery_evidence_path: Option<String>,
    pub risk_class: String,
    pub artifact_path: String,
    pub created_at: DateTime<Utc>,
}
