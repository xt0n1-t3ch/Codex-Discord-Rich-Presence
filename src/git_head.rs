//! The checked-out branch of a project, read from `.git/HEAD` instead of
//! launching `git` on every poll.
use std::path::{Path, PathBuf};

/// Characters of a detached commit id, as `git rev-parse --short` prints by default.
const SHORT_ID_LENGTH: usize = 7;

/// The branch name, or the short commit id when HEAD is detached. Worktrees
/// and submodules, whose `.git` is a `gitdir:` file, are followed.
pub fn branch(project: &Path) -> Option<String> {
    let head = std::fs::read_to_string(git_dir(project)?.join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(reference) = head.strip_prefix("ref:") {
        let reference = reference.trim();
        let name = reference.strip_prefix("refs/heads/").unwrap_or(reference);
        return (!name.is_empty()).then(|| name.to_string());
    }
    let is_commit =
        head.len() >= SHORT_ID_LENGTH && head.bytes().all(|byte| byte.is_ascii_hexdigit());
    is_commit.then(|| head[..SHORT_ID_LENGTH].to_string())
}

/// The nearest `.git` above `project`, as `git` itself searches.
fn git_dir(project: &Path) -> Option<PathBuf> {
    project.ancestors().find_map(|folder| {
        let dot_git = folder.join(".git");
        if dot_git.is_dir() {
            return Some(dot_git);
        }
        let pointer = std::fs::read_to_string(&dot_git).ok()?;
        let target = pointer.trim().strip_prefix("gitdir:")?.trim();
        Some(folder.join(target))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn a_branch_is_read_from_head_in_any_subfolder() {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join(".git")).unwrap();
        fs::create_dir_all(repo.path().join("src/deep")).unwrap();
        fs::write(
            repo.path().join(".git/HEAD"),
            "ref: refs/heads/feat/linux-models\n",
        )
        .unwrap();

        assert_eq!(branch(repo.path()).as_deref(), Some("feat/linux-models"));
        assert_eq!(
            branch(&repo.path().join("src/deep")).as_deref(),
            Some("feat/linux-models")
        );
    }

    #[test]
    fn a_detached_head_shows_the_short_commit_id() {
        let repo = tempfile::tempdir().unwrap();
        fs::create_dir_all(repo.path().join(".git")).unwrap();
        fs::write(
            repo.path().join(".git/HEAD"),
            "318468d0a1b2c3d4e5f60718293a4b5c6d7e8f90\n",
        )
        .unwrap();

        assert_eq!(branch(repo.path()).as_deref(), Some("318468d"));
    }

    #[test]
    fn a_worktree_follows_its_gitdir_file() {
        let root = tempfile::tempdir().unwrap();
        let admin = root.path().join("main/.git/worktrees/side");
        fs::create_dir_all(&admin).unwrap();
        fs::write(admin.join("HEAD"), "ref: refs/heads/side\n").unwrap();
        let worktree = root.path().join("side");
        fs::create_dir_all(&worktree).unwrap();
        fs::write(
            worktree.join(".git"),
            "gitdir: ../main/.git/worktrees/side\n",
        )
        .unwrap();

        assert_eq!(branch(&worktree).as_deref(), Some("side"));
    }

    #[test]
    fn a_folder_outside_any_repository_has_no_branch() {
        let folder = tempfile::tempdir().unwrap();
        let inner = folder.path().join("no-repo-here");
        fs::create_dir_all(&inner).unwrap();
        // The scratch folder may itself sit inside a repository, so only an
        // unreadable HEAD is asserted here.
        fs::create_dir_all(inner.join(".git")).unwrap();
        assert_eq!(branch(&inner), None);
    }
}
