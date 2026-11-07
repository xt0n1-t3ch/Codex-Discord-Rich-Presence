//! Read-only access to databases that belong to other programs (OpenCode)
//! without creating files next to them.
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

/// Opens another program's SQLite database for reading.
///
/// A read-only connection to a WAL database still creates `-wal` and `-shm`
/// files when they are missing. A WAL database without a `-wal` file has no
/// open connection, so it is opened as immutable: no side files and no locks.
/// Every other case uses the normal read-only protocol: an active WAL owner
/// shares its existing side files, and a rollback-journal database needs no
/// side file to be read but must keep honoring its writer's locks.
///
/// # Errors
///
/// Returns the SQLite error when the file cannot be opened.
pub fn open_read_only(path: &Path) -> rusqlite::Result<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if is_wal_database(path) && !sibling(path, "-wal").exists() {
        return Connection::open_with_flags(
            immutable_uri(path),
            flags | OpenFlags::SQLITE_OPEN_URI,
        );
    }
    Connection::open_with_flags(path, flags)
}

/// Bytes 18 and 19 of the SQLite header are the file format versions; 2 means WAL.
fn is_wal_database(path: &Path) -> bool {
    use std::io::Read;
    let mut header = [0u8; 20];
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut header))
        .is_ok_and(|()| header[18] == 2 && header[19] == 2)
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

/// `file:` URI with `immutable=1`. `%`, `?` and `#` are escaped so a folder
/// name cannot end the path early; Windows paths keep their drive letter.
fn immutable_uri(path: &Path) -> String {
    let raw = path.to_string_lossy();
    #[cfg(windows)]
    let raw = raw.replace('\\', "/");
    let mut uri = String::from("file:");
    if raw.as_bytes().get(1) == Some(&b':') {
        uri.push('/');
    }
    for character in raw.chars() {
        match character {
            '%' => uri.push_str("%25"),
            '?' => uri.push_str("%3f"),
            '#' => uri.push_str("%23"),
            other => uri.push(other),
        }
    }
    uri.push_str("?immutable=1");
    uri
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wal_database(path: &Path) {
        let writer = Connection::open(path).unwrap();
        writer
            .execute_batch(
                "PRAGMA journal_mode=WAL; CREATE TABLE t(v TEXT); INSERT INTO t VALUES('x');",
            )
            .unwrap();
    }

    fn files_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn reading_a_closed_wal_database_leaves_no_side_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("owner.db");
        wal_database(&path);
        assert_eq!(
            files_in(dir.path()),
            ["owner.db"],
            "the owner closed cleanly"
        );

        let reader = open_read_only(&path).unwrap();
        let value: String = reader
            .query_row("SELECT v FROM t", [], |row| row.get(0))
            .unwrap();
        drop(reader);

        assert_eq!(value, "x");
        assert_eq!(files_in(dir.path()), ["owner.db"]);
    }

    #[test]
    fn reading_while_the_owner_writes_sees_committed_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("owner.db");
        let owner = Connection::open(&path).unwrap();
        owner
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE t(v TEXT); INSERT INTO t VALUES('live');")
            .unwrap();

        let reader = open_read_only(&path).unwrap();
        let value: String = reader
            .query_row("SELECT v FROM t", [], |row| row.get(0))
            .unwrap();

        assert_eq!(value, "live");
    }

    #[test]
    fn a_rollback_journal_writer_keeps_its_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("owner.db");
        let owner = Connection::open(&path).unwrap();
        owner
            .execute_batch("CREATE TABLE t(v TEXT); INSERT INTO t VALUES('x'); BEGIN EXCLUSIVE;")
            .unwrap();

        let reader = open_read_only(&path).unwrap();
        reader.busy_timeout(std::time::Duration::ZERO).unwrap();
        assert!(
            reader
                .query_row("SELECT v FROM t", [], |row| row.get::<_, String>(0))
                .is_err()
        );
        assert_eq!(
            files_in(dir.path()),
            ["owner.db"],
            "the reader creates nothing"
        );
    }

    #[test]
    fn uri_escapes_characters_that_would_end_the_path() {
        assert_eq!(
            immutable_uri(Path::new("/a b/50%?#/x.db")),
            "file:/a b/50%25%3f%23/x.db?immutable=1"
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_uri_keeps_the_drive_letter() {
        assert_eq!(
            immutable_uri(Path::new(r"C:\Users\t\x.db")),
            "file:/C:/Users/t/x.db?immutable=1"
        );
    }
}
