use anyhow::{Context, Result};
use cfw_core::span::{DeliveryStatus, SpanRecord};
use rusqlite::{Connection, params};

use crate::paths::StorePaths;

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(paths: &StorePaths) -> Result<Self> {
        paths.ensure()?;
        let conn = Connection::open(&paths.db_path)
            .with_context(|| format!("could not open {}", paths.db_path.display()))?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;

            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                agent TEXT NOT NULL,
                repo_root TEXT,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                codex_version TEXT,
                cfw_version TEXT
            );

            CREATE TABLE IF NOT EXISTS spans (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                source TEXT NOT NULL,
                command TEXT,
                cwd TEXT,
                exit_code INTEGER,
                raw_bytes INTEGER NOT NULL,
                raw_estimated_tokens INTEGER NOT NULL,
                returned_bytes INTEGER NOT NULL,
                returned_estimated_tokens INTEGER NOT NULL,
                hash TEXT NOT NULL,
                reducer TEXT,
                policy_action TEXT NOT NULL,
                delivery_status TEXT NOT NULL,
                delivery_evidence_path TEXT,
                risk_class TEXT NOT NULL,
                artifact_path TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    pub fn insert_span(&self, span: &SpanRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO spans (
                id, session_id, kind, source, command, cwd, exit_code,
                raw_bytes, raw_estimated_tokens, returned_bytes, returned_estimated_tokens,
                hash, reducer, policy_action, delivery_status, delivery_evidence_path,
                risk_class, artifact_path, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
            "#,
            params![
                span.id,
                span.session_id,
                span.kind,
                span.source,
                span.command,
                span.cwd,
                span.exit_code,
                span.raw_bytes,
                span.raw_estimated_tokens,
                span.returned_bytes,
                span.returned_estimated_tokens,
                span.hash,
                span.reducer,
                span.policy_action,
                span.delivery_status.as_str(),
                span.delivery_evidence_path,
                span.risk_class,
                span.artifact_path,
                span.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_span(&self, id: &str) -> Result<Option<SpanRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, session_id, kind, source, command, cwd, exit_code,
                   raw_bytes, raw_estimated_tokens, returned_bytes, returned_estimated_tokens,
                   hash, reducer, policy_action, delivery_status, delivery_evidence_path,
                   risk_class, artifact_path, created_at
            FROM spans
            WHERE id = ?1
            "#,
        )?;

        let mut rows = stmt.query(params![id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };

        let status: String = row.get(14)?;
        let delivery_status = match status.as_str() {
            "replaced_tool_result" => DeliveryStatus::ReplacedToolResult,
            "advisory_wrapper" => DeliveryStatus::AdvisoryWrapper,
            "observed_only" => DeliveryStatus::ObservedOnly,
            "blocked" => DeliveryStatus::Blocked,
            _ => DeliveryStatus::Unknown,
        };
        let created_at: String = row.get(18)?;
        Ok(Some(SpanRecord {
            id: row.get(0)?,
            session_id: row.get(1)?,
            kind: row.get(2)?,
            source: row.get(3)?,
            command: row.get(4)?,
            cwd: row.get(5)?,
            exit_code: row.get(6)?,
            raw_bytes: row.get(7)?,
            raw_estimated_tokens: row.get(8)?,
            returned_bytes: row.get(9)?,
            returned_estimated_tokens: row.get(10)?,
            hash: row.get(11)?,
            reducer: row.get(12)?,
            policy_action: row.get(13)?,
            delivery_status,
            delivery_evidence_path: row.get(15)?,
            risk_class: row.get(16)?,
            artifact_path: row.get(17)?,
            created_at: chrono::DateTime::parse_from_rfc3339(&created_at)?.to_utc(),
        }))
    }

    pub fn recent_spans(&self, limit: i64) -> Result<Vec<SpanRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, session_id, kind, source, command, cwd, exit_code,
                   raw_bytes, raw_estimated_tokens, returned_bytes, returned_estimated_tokens,
                   hash, reducer, policy_action, delivery_status, delivery_evidence_path,
                   risk_class, artifact_path, created_at
            FROM spans
            ORDER BY created_at DESC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            let status: String = row.get(14)?;
            let delivery_status = match status.as_str() {
                "replaced_tool_result" => DeliveryStatus::ReplacedToolResult,
                "advisory_wrapper" => DeliveryStatus::AdvisoryWrapper,
                "observed_only" => DeliveryStatus::ObservedOnly,
                "blocked" => DeliveryStatus::Blocked,
                _ => DeliveryStatus::Unknown,
            };
            let created_at: String = row.get(18)?;
            let created_at = chrono::DateTime::parse_from_rfc3339(&created_at)
                .map_err(|err| {
                    rusqlite::Error::FromSqlConversionFailure(
                        18,
                        rusqlite::types::Type::Text,
                        Box::new(err),
                    )
                })?
                .to_utc();
            Ok(SpanRecord {
                id: row.get(0)?,
                session_id: row.get(1)?,
                kind: row.get(2)?,
                source: row.get(3)?,
                command: row.get(4)?,
                cwd: row.get(5)?,
                exit_code: row.get(6)?,
                raw_bytes: row.get(7)?,
                raw_estimated_tokens: row.get(8)?,
                returned_bytes: row.get(9)?,
                returned_estimated_tokens: row.get(10)?,
                hash: row.get(11)?,
                reducer: row.get(12)?,
                policy_action: row.get(13)?,
                delivery_status,
                delivery_evidence_path: row.get(15)?,
                risk_class: row.get(16)?,
                artifact_path: row.get(17)?,
                created_at,
            })
        })?;

        let mut spans = Vec::new();
        for row in rows {
            spans.push(row?);
        }
        Ok(spans)
    }
}
