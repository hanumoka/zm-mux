use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: String,
}

#[derive(Debug)]
pub enum WorktreeError {
    NotGitRepo,
    GitCommandFailed(String),
    Io(std::io::Error),
}

impl std::fmt::Display for WorktreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotGitRepo => write!(f, "not a git repository"),
            Self::GitCommandFailed(msg) => write!(f, "git command failed: {msg}"),
            Self::Io(e) => write!(f, "IO error: {e}"),
        }
    }
}

impl From<std::io::Error> for WorktreeError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

pub fn detect_git_root(from: &Path) -> Result<PathBuf, WorktreeError> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(from)
        .output()
        .map_err(WorktreeError::Io)?;

    if !output.status.success() {
        return Err(WorktreeError::NotGitRepo);
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(root))
}

fn short_id() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let seed = now.as_nanos() ^ (std::process::id() as u128);
    format!("{:08x}", (seed & 0xFFFF_FFFF) as u32)
}

pub fn create_worktree(
    git_root: &Path,
    agent_name: &str,
) -> Result<WorktreeInfo, WorktreeError> {
    let id = short_id();
    let branch = format!("zm/{agent_name}-{id}");
    let wt_dir = git_root.join(".zm-worktrees").join(format!("{agent_name}-{id}"));

    std::fs::create_dir_all(wt_dir.parent().unwrap())?;

    let output = Command::new("git")
        .args(["worktree", "add", "-b", &branch])
        .arg(&wt_dir)
        .current_dir(git_root)
        .output()
        .map_err(WorktreeError::Io)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WorktreeError::GitCommandFailed(stderr.trim().to_string()));
    }

    Ok(WorktreeInfo {
        path: wt_dir,
        branch,
    })
}

pub fn remove_worktree(git_root: &Path, worktree_path: &Path) -> Result<(), WorktreeError> {
    let output = Command::new("git")
        .args(["worktree", "remove", "--force"])
        .arg(worktree_path)
        .current_dir(git_root)
        .output()
        .map_err(WorktreeError::Io)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(WorktreeError::GitCommandFailed(stderr.trim().to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn init_temp_repo() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let root = tmp.path().to_path_buf();
        Command::new("git")
            .args(["init"])
            .current_dir(&root)
            .output()
            .expect("git init");
        Command::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(&root)
            .output()
            .expect("git commit");
        (tmp, root)
    }

    #[test]
    fn detect_git_root_in_repo() {
        let (_tmp, root) = init_temp_repo();
        let detected = detect_git_root(&root).expect("should detect");
        assert_eq!(
            fs::canonicalize(&detected).unwrap(),
            fs::canonicalize(&root).unwrap()
        );
    }

    #[test]
    fn detect_git_root_not_repo() {
        let tmp = tempfile::tempdir().expect("tmpdir");
        let result = detect_git_root(tmp.path());
        assert!(matches!(result, Err(WorktreeError::NotGitRepo)));
    }

    #[test]
    fn create_and_remove_worktree() {
        let (_tmp, root) = init_temp_repo();
        let wt = create_worktree(&root, "claude").expect("create");
        assert!(wt.path.exists());
        assert!(wt.branch.starts_with("zm/claude-"));

        remove_worktree(&root, &wt.path).expect("remove");
        assert!(!wt.path.exists());
    }
}
