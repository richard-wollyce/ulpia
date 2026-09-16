//! Session continuity and handoff records between sessions.
//!
//! **What this replaces.** In a stateless chat loop or between separate CLI sessions, the
//! next session starts with zero memory of what was actively being worked on, what was
//! decided, and what pending blockers remained. While `capture.rs` records recall losses
//! and `promote.rs` distills permanent knowledge notes, neither captures task continuity:
//! the active work-in-progress state that belongs to the current project and session.
//!
//! Inspired by the findings in `reports/2026-09-14-ai-memory-akita-comparado-ulpia.md`:
//! a structured handoff record holds the active task, decisions made during the session,
//! unresolved blockers, and next steps.
//!
//! ## Storage and Atomicity
//!
//! Records live under `.kb/sessions/{safe_session}.handoff.md`, written atomically via
//! `.tmp` + rename. The pointer to the most recent handoff is stored in
//! `.kb/sessions/latest-handoff.txt`.

use std::path::{Path, PathBuf};
use crate::boot::safe_session;

/// The lifecycle status of a task handoff baton.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HandoffStatus {
    #[default]
    Pending,
    Claimed,
    Completed,
}

impl HandoffStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            HandoffStatus::Pending => "pending",
            HandoffStatus::Claimed => "claimed",
            HandoffStatus::Completed => "completed",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "claimed" => HandoffStatus::Claimed,
            "completed" => HandoffStatus::Completed,
            _ => HandoffStatus::Pending,
        }
    }
}

/// A structured record of session continuity and task handoff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoffRecord {
    pub session: String,
    pub agent: String,
    pub task: String,
    pub status: HandoffStatus,
    pub claimed_by: Option<String>,
    pub decisions: Vec<String>,
    pub blockers: Vec<String>,
    pub next_steps: Vec<String>,
    pub references: Vec<String>,
    pub updated_at: String,
}

impl HandoffRecord {
    /// Renders the record as clean, human-readable Markdown with front matter.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("---\n");
        out.push_str(&format!("session: {}\n", self.session));
        out.push_str(&format!("agent: {}\n", self.agent));
        out.push_str(&format!("status: {}\n", self.status.as_str()));
        if let Some(c) = &self.claimed_by {
            out.push_str(&format!("claimed_by: {}\n", c));
        }
        out.push_str(&format!("updated_at: {}\n", self.updated_at));
        out.push_str("---\n\n");

        out.push_str("# Task\n");
        out.push_str(self.task.trim());
        out.push_str("\n\n");

        if !self.decisions.is_empty() {
            out.push_str("## Decisions\n");
            for d in &self.decisions {
                out.push_str(&format!("- {}\n", d.trim()));
            }
            out.push('\n');
        }

        if !self.blockers.is_empty() {
            out.push_str("## Blockers\n");
            for b in &self.blockers {
                out.push_str(&format!("- {}\n", b.trim()));
            }
            out.push('\n');
        }

        if !self.next_steps.is_empty() {
            out.push_str("## Next Steps\n");
            for n in &self.next_steps {
                out.push_str(&format!("- {}\n", n.trim()));
            }
            out.push('\n');
        }

        if !self.references.is_empty() {
            out.push_str("## References\n");
            for r in &self.references {
                out.push_str(&format!("- {}\n", r.trim()));
            }
            out.push('\n');
        }

        out
    }

    /// Claims the handoff baton for a specific agent. Fails if already claimed or completed.
    pub fn claim(&mut self, agent: &str) -> Result<(), String> {
        match self.status {
            HandoffStatus::Claimed => {
                let who = self.claimed_by.as_deref().unwrap_or("another agent");
                Err(format!("handoff already claimed by {who}"))
            }
            HandoffStatus::Completed => Err("handoff is already completed".to_string()),
            HandoffStatus::Pending => {
                self.status = HandoffStatus::Claimed;
                self.claimed_by = Some(agent.trim().to_string());
                Ok(())
            }
        }
    }

    /// Marks the handoff task as completed.
    pub fn complete(&mut self) -> Result<(), String> {
        if self.status == HandoffStatus::Completed {
            return Err("handoff is already completed".to_string());
        }
        self.status = HandoffStatus::Completed;
        Ok(())
    }

    /// Parses a handoff record from its Markdown representation.
    pub fn parse(text: &str) -> Option<Self> {
        let trimmed = text.trim_start();
        if !trimmed.starts_with("---") {
            return None;
        }

        let after_first = &trimmed[3..];
        let end_fm = after_first.find("\n---")?;
        let fm_block = &after_first[..end_fm];
        let body = &after_first[end_fm + 4..];

        let mut session = String::new();
        let mut agent = String::new();
        let mut status = HandoffStatus::Pending;
        let mut claimed_by: Option<String> = None;
        let mut updated_at = String::new();

        for line in fm_block.lines() {
            let line = line.trim();
            if let Some((k, v)) = line.split_once(':') {
                match k.trim() {
                    "session" => session = v.trim().to_string(),
                    "agent" => agent = v.trim().to_string(),
                    "status" => status = HandoffStatus::parse(v),
                    "claimed_by" => claimed_by = Some(v.trim().to_string()),
                    "updated_at" => updated_at = v.trim().to_string(),
                    _ => {}
                }
            }
        }

        if session.is_empty() {
            return None;
        }

        let mut decisions = Vec::new();
        let mut blockers = Vec::new();
        let mut next_steps = Vec::new();
        let mut references = Vec::new();

        let mut current_section = "";
        let mut task_lines = Vec::new();

        for line in body.lines() {
            let trimmed_line = line.trim();
            if trimmed_line.starts_with("# ") {
                let header = trimmed_line.trim_start_matches('#').trim();
                if header.eq_ignore_ascii_case("task") {
                    current_section = "task";
                } else {
                    current_section = "";
                }
                continue;
            } else if trimmed_line.starts_with("## ") {
                let header = trimmed_line.trim_start_matches('#').trim();
                if header.eq_ignore_ascii_case("decisions") {
                    current_section = "decisions";
                } else if header.eq_ignore_ascii_case("blockers") {
                    current_section = "blockers";
                } else if header.eq_ignore_ascii_case("next steps") {
                    current_section = "next";
                } else if header.eq_ignore_ascii_case("references") {
                    current_section = "references";
                } else {
                    current_section = "";
                }
                continue;
            }

            match current_section {
                "task" => {
                    if !trimmed_line.is_empty() {
                        task_lines.push(trimmed_line);
                    }
                }
                "decisions" => {
                    if let Some(item) = trimmed_line.strip_prefix("- ") {
                        decisions.push(item.trim().to_string());
                    }
                }
                "blockers" => {
                    if let Some(item) = trimmed_line.strip_prefix("- ") {
                        blockers.push(item.trim().to_string());
                    }
                }
                "next" => {
                    if let Some(item) = trimmed_line.strip_prefix("- ") {
                        next_steps.push(item.trim().to_string());
                    }
                }
                "references" => {
                    if let Some(item) = trimmed_line.strip_prefix("- ") {
                        references.push(item.trim().to_string());
                    }
                }
                _ => {}
            }
        }

        let task = task_lines.join(" ");

        Some(HandoffRecord {
            session,
            agent,
            task,
            status,
            claimed_by,
            decisions,
            blockers,
            next_steps,
            references,
            updated_at,
        })
    }

    /// Formats a concise summary of the handoff to inject into model context on boot.
    pub fn format_for_briefing(&self, max_chars: usize) -> String {
        let mut out = format!(
            "VESTA: CONTINUITY (session {}, status: {}):\n",
            self.session,
            self.status.as_str()
        );
        if !self.agent.is_empty() {
            out.push_str(&format!("  Agent: {}\n", self.agent));
        }
        if let Some(claimer) = &self.claimed_by {
            out.push_str(&format!("  Claimed by: {}\n", claimer));
        }
        if !self.task.is_empty() {
            out.push_str(&format!("  Active task: {}\n", self.task));
        }
        if !self.decisions.is_empty() {
            out.push_str("  Decisions:\n");
            for d in &self.decisions {
                out.push_str(&format!("    - {}\n", d));
            }
        }
        if !self.blockers.is_empty() {
            out.push_str("  Blockers:\n");
            for b in &self.blockers {
                out.push_str(&format!("    - {}\n", b));
            }
        }
        if !self.next_steps.is_empty() {
            out.push_str("  Next steps:\n");
            for n in &self.next_steps {
                out.push_str(&format!("    - {}\n", n));
            }
        }

        if out.len() > max_chars {
            let mut cut = out[..max_chars].to_string();
            cut.push_str("\n  [...continuity truncated to budget]\n");
            cut
        } else {
            out
        }
    }

    /// Converts the record into a JSON value representation.
    pub fn as_json(&self) -> crate::json::Value {
        let mut obj = crate::json::Value::obj();
        obj.set("session", crate::json::Value::Str(self.session.clone()));
        obj.set("agent", crate::json::Value::Str(self.agent.clone()));
        obj.set("task", crate::json::Value::Str(self.task.clone()));
        obj.set("status", crate::json::Value::Str(self.status.as_str().to_string()));
        if let Some(c) = &self.claimed_by {
            obj.set("claimed_by", crate::json::Value::Str(c.clone()));
        }
        obj.set(
            "decisions",
            crate::json::Value::Arr(self.decisions.iter().cloned().map(crate::json::Value::Str).collect()),
        );
        obj.set(
            "blockers",
            crate::json::Value::Arr(self.blockers.iter().cloned().map(crate::json::Value::Str).collect()),
        );
        obj.set(
            "next_steps",
            crate::json::Value::Arr(self.next_steps.iter().cloned().map(crate::json::Value::Str).collect()),
        );
        obj.set(
            "references",
            crate::json::Value::Arr(self.references.iter().cloned().map(crate::json::Value::Str).collect()),
        );
        obj.set("updated_at", crate::json::Value::Str(self.updated_at.clone()));
        obj
    }
}

/// Directory holding session handoff files: `.kb/sessions/`
pub fn handoff_dir(root: &Path) -> PathBuf {
    root.join(".kb").join("sessions")
}

/// Path for a specific session's handoff file.
pub fn handoff_file(root: &Path, session: &str) -> PathBuf {
    handoff_dir(root).join(format!("{}.handoff.md", safe_session(session)))
}

/// Pointer file indicating the latest session with a handoff record.
pub fn latest_pointer(root: &Path) -> PathBuf {
    handoff_dir(root).join("latest-handoff.txt")
}

/// Atomically saves a handoff record to `.kb/sessions/{session}.handoff.md`
/// and updates `.kb/sessions/latest-handoff.txt`.
pub fn save(root: &Path, record: &HandoffRecord) -> std::io::Result<PathBuf> {
    let dir = handoff_dir(root);
    std::fs::create_dir_all(&dir)?;

    let target = handoff_file(root, &record.session);
    let tmp = dir.join(format!("{}.handoff.md.tmp", safe_session(&record.session)));

    let content = record.render();
    std::fs::write(&tmp, &content)?;
    std::fs::rename(&tmp, &target)?;

    // Update latest pointer atomically
    let ptr_target = latest_pointer(root);
    let ptr_tmp = dir.join("latest-handoff.txt.tmp");
    std::fs::write(&ptr_tmp, record.session.trim())?;
    let _ = std::fs::rename(&ptr_tmp, &ptr_target);

    Ok(target)
}

/// Loads a handoff record for a given session id.
pub fn load(root: &Path, session: &str) -> Option<HandoffRecord> {
    let path = handoff_file(root, session);
    let text = std::fs::read_to_string(path).ok()?;
    HandoffRecord::parse(&text)
}

/// Loads the most recent handoff record, if any.
pub fn latest(root: &Path) -> Option<HandoffRecord> {
    let ptr = latest_pointer(root);
    if let Ok(session_id) = std::fs::read_to_string(ptr) {
        let trimmed = session_id.trim();
        if !trimmed.is_empty() {
            if let Some(record) = load(root, trimmed) {
                return Some(record);
            }
        }
    }

    // Fallback: scan `.kb/sessions/` for the latest `.handoff.md` by mtime
    let dir = handoff_dir(root);
    let entries = std::fs::read_dir(dir).ok()?;
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;

    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().is_some_and(|e| e == "md")
            && p.file_name().is_some_and(|f| f.to_string_lossy().ends_with(".handoff.md"))
        {
            if let Ok(meta) = p.metadata() {
                if let Ok(mtime) = meta.modified() {
                    match &best {
                        Some((cur_mtime, _)) if mtime > *cur_mtime => {
                            best = Some((mtime, p));
                        }
                        None => {
                            best = Some((mtime, p));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    let (_, path) = best?;
    let text = std::fs::read_to_string(path).ok()?;
    HandoffRecord::parse(&text)
}

/// Lists all handoff records from `.kb/sessions/`, optionally filtered by agent,
/// sorted by modified time (most recent first).
pub fn list_handoffs(root: &Path, agent_filter: Option<&str>) -> Vec<HandoffRecord> {
    let dir = handoff_dir(root);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut records: Vec<(std::time::SystemTime, HandoffRecord)> = Vec::new();

    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().is_some_and(|e| e == "md")
            && p.file_name().is_some_and(|f| f.to_string_lossy().ends_with(".handoff.md"))
        {
            if let Ok(text) = std::fs::read_to_string(&p) {
                if let Some(record) = HandoffRecord::parse(&text) {
                    if let Some(agent) = agent_filter {
                        if !record.agent.eq_ignore_ascii_case(agent) {
                            continue;
                        }
                    }
                    let mtime = p
                        .metadata()
                        .and_then(|m| m.modified())
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    records.push((mtime, record));
                }
            }
        }
    }

    records.sort_by(|a, b| b.0.cmp(&a.0));
    records.into_iter().map(|(_, r)| r).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handoff_roundtrip_render_and_parse() {
        let original = HandoffRecord {
            session: "sess-1234".into(),
            agent: "zed".into(),
            task: "Implement Phase 2 handoff records with TDD".into(),
            status: HandoffStatus::Claimed,
            claimed_by: Some("cicero".into()),
            decisions: vec![
                "Store handoffs in .kb/sessions/*.handoff.md".into(),
                "Use atomic .tmp + rename to prevent corruption".into(),
            ],
            blockers: vec!["Awaiting user approval for Phase 3".into()],
            next_steps: vec![
                "Wire handoff into boot::brief".into(),
                "Add CLI command kb handoff".into(),
            ],
            references: vec!["reports/2026-09-14-ai-memory-akita-comparado-ulpia.md".into()],
            updated_at: "2026-09-14T20:50:00Z".into(),
        };

        let rendered = original.render();
        let parsed = HandoffRecord::parse(&rendered).expect("failed to parse rendered handoff");

        assert_eq!(parsed.session, original.session);
        assert_eq!(parsed.agent, original.agent);
        assert_eq!(parsed.task, original.task);
        assert_eq!(parsed.status, original.status);
        assert_eq!(parsed.claimed_by, original.claimed_by);
        assert_eq!(parsed.decisions, original.decisions);
        assert_eq!(parsed.blockers, original.blockers);
        assert_eq!(parsed.next_steps, original.next_steps);
        assert_eq!(parsed.references, original.references);
        assert_eq!(parsed.updated_at, original.updated_at);
    }

    #[test]
    fn handoff_claim_and_complete_lifecycle() {
        let mut record = HandoffRecord {
            session: "lifecycle-sess".into(),
            agent: "zed".into(),
            task: "Multi-agent task handoff".into(),
            status: HandoffStatus::Pending,
            claimed_by: None,
            decisions: vec![],
            blockers: vec![],
            next_steps: vec![],
            references: vec![],
            updated_at: "2026-09-16".into(),
        };

        assert_eq!(record.status, HandoffStatus::Pending);
        assert!(record.claimed_by.is_none());

        // First claim succeeds
        assert!(record.claim("cicero").is_ok());
        assert_eq!(record.status, HandoffStatus::Claimed);
        assert_eq!(record.claimed_by.as_deref(), Some("cicero"));

        // Second claim by another agent fails (already claimed)
        let second_claim = record.claim("frontinus");
        assert!(second_claim.is_err());
        assert!(second_claim.unwrap_err().contains("already claimed by cicero"));

        // Complete transitions to Completed
        assert!(record.complete().is_ok());
        assert_eq!(record.status, HandoffStatus::Completed);

        // Claim on completed handoff fails
        let third_claim = record.claim("yaron");
        assert!(third_claim.is_err());
        assert!(third_claim.unwrap_err().contains("already completed"));
    }

    #[test]
    fn handoff_atomic_save_and_load() {
        let temp = std::env::temp_dir().join(format!("kb-test-handoff-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();

        let record = HandoffRecord {
            session: "test-session-abc".into(),
            agent: "zed".into(),
            task: "Verify atomic write".into(),
            status: HandoffStatus::Pending,
            claimed_by: None,
            decisions: vec!["Checked with TDD".into()],
            blockers: Vec::new(),
            next_steps: vec!["Run cargo test".into()],
            references: Vec::new(),
            updated_at: "2026-09-14".into(),
        };

        let path = save(&temp, &record).expect("save failed");
        assert!(path.exists(), "target file must exist");
        assert!(!temp.join(".kb/sessions/test-session-abc.handoff.md.tmp").exists(), "tmp file must be gone");

        let loaded = load(&temp, "test-session-abc").expect("load failed");
        assert_eq!(loaded.task, "Verify atomic write");
        assert_eq!(loaded.agent, "zed");
        assert_eq!(loaded.status, HandoffStatus::Pending);

        let latest_rec = latest(&temp).expect("latest failed");
        assert_eq!(latest_rec.session, "test-session-abc");

        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn handoff_format_budget() {
        let record = HandoffRecord {
            session: "budget-session".into(),
            agent: "zed".into(),
            task: "A very long task description that might exceed budget".into(),
            status: HandoffStatus::Claimed,
            claimed_by: Some("frontinus".into()),
            decisions: vec!["Decision 1".into(), "Decision 2".into()],
            blockers: Vec::new(),
            next_steps: vec!["Step 1".into()],
            references: Vec::new(),
            updated_at: "2026-09-14".into(),
        };

        let formatted_full = record.format_for_briefing(2000);
        assert!(formatted_full.contains("VESTA: CONTINUITY"));
        assert!(formatted_full.contains("status: claimed"));
        assert!(formatted_full.contains("Claimed by: frontinus"));
        assert!(formatted_full.contains("Decision 1"));

        let formatted_truncated = record.format_for_briefing(50);
        assert!(formatted_truncated.contains("[...continuity truncated to budget]"));
    }

    #[test]
    fn handoff_as_json_representation() {
        let record = HandoffRecord {
            session: "json-sess".into(),
            agent: "zed".into(),
            task: "Test json serialization".into(),
            status: HandoffStatus::Claimed,
            claimed_by: Some("yaron".into()),
            decisions: vec!["Decision A".into()],
            blockers: vec!["None".into()],
            next_steps: vec!["Verify json".into()],
            references: vec!["note.md".into()],
            updated_at: "2026-09-14".into(),
        };

        let json_val = record.as_json();
        assert_eq!(json_val.get("session"), Some(&crate::json::Value::Str("json-sess".into())));
        assert_eq!(json_val.get("agent"), Some(&crate::json::Value::Str("zed".into())));
        assert_eq!(json_val.get("status"), Some(&crate::json::Value::Str("claimed".into())));
        assert_eq!(json_val.get("claimed_by"), Some(&crate::json::Value::Str("yaron".into())));
        assert_eq!(json_val.get("task"), Some(&crate::json::Value::Str("Test json serialization".into())));
        let serialized = json_val.to_string();
        assert!(serialized.contains("\"session\":\"json-sess\""));
        assert!(serialized.contains("\"status\":\"claimed\""));
    }

    #[test]
    fn handoff_list_and_filter() {
        let temp = std::env::temp_dir().join(format!("kb-test-handoff-list-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();

        let r1 = HandoffRecord {
            session: "s-1".into(),
            agent: "zed".into(),
            task: "Task 1".into(),
            status: HandoffStatus::Pending,
            claimed_by: None,
            decisions: vec![],
            blockers: vec![],
            next_steps: vec![],
            references: vec![],
            updated_at: "2026-09-14".into(),
        };
        let r2 = HandoffRecord {
            session: "s-2".into(),
            agent: "cicero".into(),
            task: "Task 2".into(),
            status: HandoffStatus::Completed,
            claimed_by: Some("cicero".into()),
            decisions: vec![],
            blockers: vec![],
            next_steps: vec![],
            references: vec![],
            updated_at: "2026-09-14".into(),
        };

        save(&temp, &r1).unwrap();
        save(&temp, &r2).unwrap();

        let all = list_handoffs(&temp, None);
        assert_eq!(all.len(), 2);

        let zed_only = list_handoffs(&temp, Some("zed"));
        assert_eq!(zed_only.len(), 1);
        assert_eq!(zed_only[0].session, "s-1");

        let _ = std::fs::remove_dir_all(&temp);
    }
}
