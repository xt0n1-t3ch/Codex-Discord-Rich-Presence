//! Finds the session files to read on each poll without walking the whole
//! history every time.
//!
//! Codex stores sessions as `sessions/YYYY/MM/DD/*.jsonl`. A full walk of every
//! folder runs at most once per [`FULL_WALK_INTERVAL`]. Polls in between read
//! only today's and yesterday's folders plus the files the last full walk
//! found recent, so the cost of a poll does not grow with the history.
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use chrono::{Local, NaiveDate, Utc};
use walkdir::WalkDir;

/// How often a full walk runs. `codex resume` appends to the session's file
/// in its original day folder, so a resumed session appears within this time.
pub const FULL_WALK_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Default)]
pub struct SessionWalk {
    last_full: Option<Instant>,
    /// Files kept after the last full walk; re-checked on every short walk.
    hot: Vec<PathBuf>,
}

impl SessionWalk {
    /// The `.jsonl` files to consider now. `full` is true when every folder was read.
    pub fn candidates(&mut self, roots: &[PathBuf], now: Instant) -> (Vec<PathBuf>, bool) {
        let full = self
            .last_full
            .is_none_or(|last| now.saturating_duration_since(last) >= FULL_WALK_INTERVAL);
        if full {
            self.last_full = Some(now);
            let files = roots
                .iter()
                .flat_map(|root| jsonl_files(root, None))
                .collect();
            return (files, true);
        }
        let mut files = self.hot.to_vec();
        for root in roots {
            for day in recent_days() {
                let folder = root.join(day.format("%Y/%m/%d").to_string());
                files.extend(jsonl_files(&folder, Some(1)));
            }
        }
        files.sort();
        files.dedup();
        (files, false)
    }

    /// Makes the next poll a full walk.
    #[cfg(test)]
    pub fn expire(&mut self) {
        self.last_full = None;
    }

    /// Records the files a full walk kept, for the short walks that follow.
    pub fn remember(&mut self, kept: impl IntoIterator<Item = PathBuf>) {
        self.hot = kept.into_iter().collect();
    }
}

/// Today and yesterday in local time and in UTC, so a folder named in either
/// zone is read around midnight.
fn recent_days() -> Vec<NaiveDate> {
    let mut days: Vec<NaiveDate> = [Local::now().date_naive(), Utc::now().date_naive()]
        .into_iter()
        .flat_map(|day| [day, day.pred_opt().unwrap_or(day)])
        .collect();
    days.sort();
    days.dedup();
    days
}

fn jsonl_files(root: &Path, max_depth: Option<usize>) -> Vec<PathBuf> {
    if !root.exists() {
        return Vec::new();
    }
    let walk = WalkDir::new(root);
    let walk = match max_depth {
        Some(depth) => walk.max_depth(depth),
        None => walk,
    };
    walk.into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("jsonl"))
        .map(walkdir::DirEntry::into_path)
        .collect()
}

/// Modification time of a candidate, or `None` when it vanished.
pub fn modified(path: &Path) -> Option<(std::fs::Metadata, SystemTime)> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    metadata.is_file().then_some((metadata, modified))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn day_folder(root: &Path, day: NaiveDate) -> PathBuf {
        let folder = root.join(day.format("%Y/%m/%d").to_string());
        fs::create_dir_all(&folder).unwrap();
        folder
    }

    #[test]
    fn short_walks_read_recent_days_and_known_files_only() {
        let root = tempfile::tempdir().unwrap();
        let roots = [root.path().to_path_buf()];
        let today = Local::now().date_naive();
        let old_day = today - chrono::Days::new(30);
        let known_old = day_folder(root.path(), old_day).join("resumed.jsonl");
        fs::write(&known_old, "{}").unwrap();
        let mut walk = SessionWalk::default();
        let start = Instant::now();

        let (first, full) = walk.candidates(&roots, start);
        assert!(full, "the first poll reads every folder");
        assert_eq!(first, std::slice::from_ref(&known_old));
        walk.remember(first);

        let new_today = day_folder(root.path(), today).join("new.jsonl");
        fs::write(&new_today, "{}").unwrap();
        let unseen_old = day_folder(root.path(), old_day).join("other.jsonl");
        fs::write(&unseen_old, "{}").unwrap();

        let (short, full) = walk.candidates(&roots, start + Duration::from_secs(2));
        assert!(!full);
        assert!(
            short.contains(&new_today),
            "today's folder is read every poll"
        );
        assert!(
            short.contains(&known_old),
            "a recent file from the last full walk stays"
        );
        assert!(
            !short.contains(&unseen_old),
            "old folders wait for the next full walk"
        );

        let (later, full) = walk.candidates(&roots, start + FULL_WALK_INTERVAL);
        assert!(full);
        assert!(later.contains(&unseen_old));
    }
}
