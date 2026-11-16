//! # gbade-git
//!
//! Version control for GBADE projects, the beginner-friendly subset of git:
//!
//! - start tracking a project (`git init` + `.gitignore` + a first commit),
//! - see which files changed since the last commit, and how (a diff),
//! - see which lines of the open file changed (editor margin),
//! - save a snapshot of everything ("commit all"),
//! - browse the history and what each commit changed,
//! - throw away the changes to one file.
//!
//! Built on libgit2 (through the `git2` crate), compiled into GBADE: no git
//! installation is needed, and repositories are normal git repositories
//! that any other git tool can open. Network operations (push/pull) are not
//! part of this crate.

use git2::{DiffOptions, IndexAddOption, Repository, Signature, Status, StatusOptions};
use std::path::{Path, PathBuf};

pub use git2::Error;

/// How a file differs from the last commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FileState {
    /// Not in the last commit yet.
    New,
    Modified,
    Deleted,
    Renamed,
    /// Merge conflict (only happens if other git tools were used).
    Conflicted,
}

impl FileState {
    /// One-letter code, like `git status --short`.
    pub fn letter(self) -> char {
        match self {
            FileState::New => 'A',
            FileState::Modified => 'M',
            FileState::Deleted => 'D',
            FileState::Renamed => 'R',
            FileState::Conflicted => 'C',
        }
    }
    pub fn describe(self) -> &'static str {
        match self {
            FileState::New => "new file",
            FileState::Modified => "modified",
            FileState::Deleted => "deleted",
            FileState::Renamed => "renamed",
            FileState::Conflicted => "conflict",
        }
    }
}

/// A changed file, relative to the repository root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub path: PathBuf,
    pub state: FileState,
}

/// One line of a diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    /// '+' added, '-' removed, ' ' unchanged context.
    pub origin: char,
    pub text: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
}

/// The diff of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: PathBuf,
    pub state: FileState,
    /// Hunk headers ("@@ -1,4 +1,5 @@") are given as lines with origin '@'.
    pub lines: Vec<DiffLine>,
    pub binary: bool,
}

/// How a line of the working file differs from the last commit, for the
/// editor margin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineChange {
    Added,
    Modified,
    /// Lines were removed just before this line.
    DeletedAbove,
}

/// A commit in the history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    pub id: String,
    pub short_id: String,
    pub summary: String,
    pub message: String,
    pub author: String,
    /// Seconds since the Unix epoch.
    pub time: i64,
}

/// An open repository.
pub struct Repo {
    repo: Repository,
    root: PathBuf,
}

/// The user's global git identity (`~/.gitconfig`), if they have one, to
/// pre-fill GBADE's fields. GBADE itself only ever writes repository-local
/// settings.
pub fn global_identity() -> Option<(String, String)> {
    let cfg = git2::Config::open_default().ok()?;
    let name = cfg.get_string("user.name").ok().filter(|s| !s.trim().is_empty())?;
    let email = cfg.get_string("user.email").ok().filter(|s| !s.trim().is_empty())?;
    Some((name, email))
}

/// What GBADE puts in a new repository's `.gitignore`.
pub const GITIGNORE: &str = "# Created by GBADE: build output is recreated by every build.\nbuild/\n.DS_Store\n";

impl Repo {
    /// Open the repository containing `dir`, if any.
    pub fn open(dir: &Path) -> Option<Repo> {
        let repo = Repository::discover(dir).ok()?;
        let root = dunce::canonicalize(repo.workdir()?).ok()?;
        Some(Repo { repo, root })
    }

    /// Start tracking `dir`: create the repository, a `.gitignore` that
    /// excludes `build/`, and a first commit with every file.
    pub fn init(dir: &Path, name: &str, email: &str) -> Result<Repo, Error> {
        let repo = Repository::init(dir)?;
        let root = repo.workdir().map(Path::to_path_buf).unwrap_or_else(|| dir.to_path_buf());
        let root = dunce::canonicalize(&root).unwrap_or(root);
        let gi = root.join(".gitignore");
        if !gi.exists() {
            std::fs::write(&gi, GITIGNORE).map_err(|e| Error::from_str(&e.to_string()))?;
        }
        let r = Repo { repo, root };
        if r.identity().is_none() {
            r.set_identity(name, email)?;
        }
        r.commit_all("Start tracking this project with GBADE")?;
        Ok(r)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Name of the current branch ("main", "master"...), or a short commit id
    /// when no branch is checked out.
    pub fn branch(&self) -> String {
        match self.repo.head() {
            Ok(h) if h.is_branch() => h.shorthand().unwrap_or("?").to_string(),
            Ok(h) => h.target().map(|o| o.to_string()[..7].to_string()).unwrap_or_default(),
            Err(_) => "main".into(), // no commit yet
        }
    }

    /// The configured author (`user.name` / `user.email`), if any.
    pub fn identity(&self) -> Option<(String, String)> {
        let cfg = self.repo.config().ok()?;
        let name = cfg.get_string("user.name").ok().filter(|s| !s.trim().is_empty())?;
        let email = cfg.get_string("user.email").ok().filter(|s| !s.trim().is_empty())?;
        Some((name, email))
    }

    /// Save the author in this repository's own config (`.git/config`), so
    /// GBADE never touches the user's global git settings.
    pub fn set_identity(&self, name: &str, email: &str) -> Result<(), Error> {
        let mut cfg = self.repo.config()?.open_level(git2::ConfigLevel::Local)?;
        cfg.set_str("user.name", name.trim())?;
        cfg.set_str("user.email", email.trim())?;
        Ok(())
    }

    /// Every file that differs from the last commit (ignored files excluded).
    pub fn status(&self) -> Result<Vec<FileChange>, Error> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true).recurse_untracked_dirs(true).include_ignored(false).renames_head_to_index(true);
        let statuses = self.repo.statuses(Some(&mut opts))?;
        let mut out = Vec::new();
        for s in statuses.iter() {
            let st = s.status();
            let state = if st.contains(Status::CONFLICTED) {
                FileState::Conflicted
            } else if st.intersects(Status::WT_NEW | Status::INDEX_NEW) {
                FileState::New
            } else if st.intersects(Status::WT_DELETED | Status::INDEX_DELETED) {
                FileState::Deleted
            } else if st.intersects(Status::WT_RENAMED | Status::INDEX_RENAMED) {
                FileState::Renamed
            } else if st.intersects(Status::WT_MODIFIED | Status::INDEX_MODIFIED | Status::WT_TYPECHANGE | Status::INDEX_TYPECHANGE) {
                FileState::Modified
            } else {
                continue;
            };
            if let Ok(p) = s.path() {
                out.push(FileChange { path: PathBuf::from(p), state });
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    /// Absolute path -> path relative to the repository (as git stores it).
    pub fn relative(&self, path: &Path) -> Option<PathBuf> {
        let abs = dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        abs.strip_prefix(&self.root).ok().map(Path::to_path_buf).or_else(|| {
            // A deleted file cannot be canonicalised: compare the parent.
            let parent = dunce::canonicalize(path.parent()?).ok()?;
            Some(parent.strip_prefix(&self.root).ok()?.join(path.file_name()?))
        })
    }

    /// The content of `rel` in the last commit (None if new or no commit).
    pub fn head_content(&self, rel: &Path) -> Option<Vec<u8>> {
        let tree = self.repo.head().ok()?.peel_to_tree().ok()?;
        let entry = tree.get_path(rel).ok()?;
        let blob = entry.to_object(&self.repo).ok()?.peel_to_blob().ok()?;
        Some(blob.content().to_vec())
    }

    /// Diff of the working copy against the last commit, for every changed file.
    pub fn working_diff(&self) -> Result<Vec<FileDiff>, Error> {
        let head_tree = self.repo.head().ok().and_then(|h| h.peel_to_tree().ok());
        let mut opts = DiffOptions::new();
        opts.include_untracked(true).recurse_untracked_dirs(true).show_untracked_content(true).context_lines(3);
        let diff = self.repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts))?;
        collect_diff(&diff)
    }

    /// Diff of one file of the working copy against the last commit.
    pub fn file_diff(&self, rel: &Path) -> Result<Option<FileDiff>, Error> {
        Ok(self.working_diff()?.into_iter().find(|d| d.path == rel))
    }

    /// Which lines of `text` (the editor's current, maybe unsaved, content of
    /// `rel`) differ from the last commit. 1-based line numbers.
    pub fn line_changes(&self, rel: &Path, text: &str) -> Vec<(u32, LineChange)> {
        let Some(old) = self.head_content(rel) else {
            // Not committed yet: every line is new.
            if self.repo.head().is_ok() {
                return (1..=text.lines().count() as u32).map(|l| (l, LineChange::Added)).collect();
            }
            return Vec::new();
        };
        let mut opts = DiffOptions::new();
        opts.context_lines(0);
        let mut out: Vec<(u32, LineChange)> = Vec::new();
        let patch = git2::Patch::from_buffers(&old, None, text.as_bytes(), None, Some(&mut opts));
        let Ok(patch) = patch else { return out };
        for h in 0..patch.num_hunks() {
            let Ok((hunk, _)) = patch.hunk(h) else { continue };
            let removed = hunk.old_lines();
            let added = hunk.new_lines();
            let start = hunk.new_start();
            if added == 0 {
                // Pure deletion: mark the line after it.
                out.push((start.max(0) + 1, LineChange::DeletedAbove));
                continue;
            }
            let kind = if removed == 0 { LineChange::Added } else { LineChange::Modified };
            for l in start..start + added {
                out.push((l, kind));
            }
        }
        out
    }

    /// Stage every change (including new and deleted files) and commit.
    pub fn commit_all(&self, message: &str) -> Result<String, Error> {
        let message = message.trim();
        if message.is_empty() {
            return Err(Error::from_str("write a message describing the change first"));
        }
        let mut index = self.repo.index()?;
        index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
        index.update_all(["*"].iter(), None)?;
        index.write()?;
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;
        let sig = match self.identity() {
            Some((n, e)) => Signature::now(&n, &e)?,
            None => return Err(Error::from_str("set your name and email before the first commit")),
        };
        let parent = self.repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        if let Some(p) = &parent {
            if p.tree_id() == tree_id {
                return Err(Error::from_str("nothing changed since the last commit"));
            }
        }
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        let id = self.repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;
        Ok(id.to_string())
    }

    /// The most recent commits, newest first.
    pub fn history(&self, limit: usize) -> Result<Vec<CommitInfo>, Error> {
        let mut walk = self.repo.revwalk()?;
        if walk.push_head().is_err() {
            return Ok(Vec::new()); // no commit yet
        }
        walk.set_sorting(git2::Sort::TIME)?;
        let mut out = Vec::new();
        for oid in walk.take(limit) {
            let c = self.repo.find_commit(oid?)?;
            let id = c.id().to_string();
            out.push(CommitInfo {
                short_id: id[..7].to_string(),
                id,
                summary: c.summary().ok().flatten().unwrap_or("").to_string(),
                message: c.message().unwrap_or("").to_string(),
                author: c.author().name().unwrap_or("").to_string(),
                time: c.time().seconds(),
            });
        }
        Ok(out)
    }

    /// What a commit changed, compared with its parent.
    pub fn commit_diff(&self, id: &str) -> Result<Vec<FileDiff>, Error> {
        let c = self.repo.find_commit(git2::Oid::from_str(id)?)?;
        let tree = c.tree()?;
        let parent_tree = c.parent(0).ok().and_then(|p| p.tree().ok());
        let mut opts = DiffOptions::new();
        opts.context_lines(3);
        let diff = self.repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut opts))?;
        collect_diff(&diff)
    }

    /// Throw away the changes to one file: restore it as in the last commit,
    /// or delete it if it is new. Cannot be undone.
    pub fn discard(&self, rel: &Path) -> Result<(), Error> {
        match self.head_content(rel) {
            Some(content) => {
                let abs = self.root.join(rel);
                if let Some(parent) = abs.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| Error::from_str(&e.to_string()))?;
                }
                std::fs::write(&abs, content).map_err(|e| Error::from_str(&e.to_string()))?;
            }
            None => {
                let abs = self.root.join(rel);
                if abs.exists() {
                    std::fs::remove_file(&abs).map_err(|e| Error::from_str(&e.to_string()))?;
                }
            }
        }
        // Keep the index consistent with HEAD for this path.
        let mut index = self.repo.index()?;
        if let Ok(head) = self.repo.head().and_then(|h| h.peel_to_commit()) {
            self.repo.reset_default(Some(head.as_object()), [rel].iter())?;
        } else {
            let _ = index.remove_path(rel);
            index.write()?;
        }
        Ok(())
    }
}

fn collect_diff(diff: &git2::Diff) -> Result<Vec<FileDiff>, Error> {
    let mut files: Vec<FileDiff> = Vec::new();
    diff.print(git2::DiffFormat::Patch, |delta, hunk, line| {
        let path = delta.new_file().path().or_else(|| delta.old_file().path()).map(Path::to_path_buf).unwrap_or_default();
        if files.last().map(|f| &f.path) != Some(&path) {
            let state = match delta.status() {
                git2::Delta::Added | git2::Delta::Untracked => FileState::New,
                git2::Delta::Deleted => FileState::Deleted,
                git2::Delta::Renamed => FileState::Renamed,
                git2::Delta::Conflicted => FileState::Conflicted,
                _ => FileState::Modified,
            };
            files.push(FileDiff { path: path.clone(), state, lines: Vec::new(), binary: delta.flags().is_binary() });
        }
        let f = files.last_mut().unwrap();
        match line.origin() {
            '+' | '-' | ' ' => f.lines.push(DiffLine {
                origin: line.origin(),
                text: String::from_utf8_lossy(line.content()).trim_end_matches(['\n', '\r']).to_string(),
                old_line: line.old_lineno(),
                new_line: line.new_lineno(),
            }),
            'H' => {
                if let Some(h) = hunk {
                    f.lines.push(DiffLine {
                        origin: '@',
                        text: String::from_utf8_lossy(h.header()).trim_end().to_string(),
                        old_line: None,
                        new_line: None,
                    });
                }
            }
            'B' => f.binary = true,
            _ => {}
        }
        true
    })?;
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Make the tests independent of the machine's git settings: libgit2
    /// looks for the global, XDG, system and ProgramData configuration in an
    /// empty folder, so a developer's (or a CI runner's) `user.name` does
    /// not leak into the repositories the tests create.
    fn isolate_git_config() {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            let empty = tempfile::tempdir().unwrap().keep();
            for level in [git2::ConfigLevel::Global, git2::ConfigLevel::XDG, git2::ConfigLevel::System, git2::ConfigLevel::ProgramData] {
                // SAFETY: called once, before any repository is opened.
                unsafe { git2::opts::set_search_path(level, empty.as_path()) }.unwrap();
            }
        });
    }

    fn project() -> (tempfile::TempDir, PathBuf) {
        isolate_git_config();
        let dir = tempfile::tempdir().unwrap();
        let root = dunce::canonicalize(dir.path()).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("build")).unwrap();
        std::fs::write(root.join("gbade.toml"), "[game]\nname = \"T\"\n").unwrap();
        std::fs::write(root.join("src/main.c"), "int a;\nint b;\nint c;\n").unwrap();
        std::fs::write(root.join("build/game.gba"), b"\x00\x01ROM").unwrap();
        (dir, root)
    }

    #[test]
    fn init_creates_first_commit_and_ignores_build() {
        let (_d, root) = project();
        assert!(Repo::open(&root).is_none());
        let r = Repo::init(&root, "Ada", "ada@example.com").unwrap();
        assert!(root.join(".git").is_dir());
        assert_eq!(std::fs::read_to_string(root.join(".gitignore")).unwrap(), GITIGNORE);
        assert!(r.status().unwrap().is_empty(), "everything committed, build/ ignored");
        let h = r.history(10).unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].author, "Ada");
        assert!(r.head_content(Path::new("src/main.c")).is_some());
        assert!(r.head_content(Path::new("build/game.gba")).is_none());
        assert_eq!(r.identity(), Some(("Ada".into(), "ada@example.com".into())));
        // Reopen from a subfolder.
        assert_eq!(Repo::open(&root.join("src")).unwrap().root(), root);
    }

    #[test]
    fn status_diff_commit_and_history() {
        let (_d, root) = project();
        let r = Repo::init(&root, "Ada", "ada@example.com").unwrap();
        std::fs::write(root.join("src/main.c"), "int a;\nint B;\nint c;\nint d;\n").unwrap();
        std::fs::write(root.join("src/new.c"), "void f(void) {}\n").unwrap();
        std::fs::remove_file(root.join("gbade.toml")).unwrap();
        let st = r.status().unwrap();
        let find = |p: &str| st.iter().find(|c| c.path == Path::new(p)).map(|c| c.state);
        assert_eq!(find("src/main.c"), Some(FileState::Modified));
        assert_eq!(find("src/new.c"), Some(FileState::New));
        assert_eq!(find("gbade.toml"), Some(FileState::Deleted));
        assert_eq!(st.len(), 3);

        let d = r.file_diff(Path::new("src/main.c")).unwrap().unwrap();
        let removed: Vec<&str> = d.lines.iter().filter(|l| l.origin == '-').map(|l| l.text.as_str()).collect();
        let added: Vec<&str> = d.lines.iter().filter(|l| l.origin == '+').map(|l| l.text.as_str()).collect();
        assert_eq!(removed, vec!["int b;"]);
        assert_eq!(added, vec!["int B;", "int d;"]);
        assert!(d.lines.iter().any(|l| l.origin == '@'));
        let new = r.file_diff(Path::new("src/new.c")).unwrap().unwrap();
        assert_eq!(new.state, FileState::New);

        assert!(r.commit_all("   ").is_err(), "empty message refused");
        let id = r.commit_all("Rename b, add d and new.c").unwrap();
        assert!(r.status().unwrap().is_empty());
        assert!(r.commit_all("again").is_err(), "nothing to commit");
        let h = r.history(10).unwrap();
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].id, id);
        assert_eq!(h[0].summary, "Rename b, add d and new.c");
        let cd = r.commit_diff(&id).unwrap();
        let paths: Vec<_> = cd.iter().map(|f| (f.path.to_string_lossy().to_string(), f.state)).collect();
        assert!(paths.contains(&("gbade.toml".into(), FileState::Deleted)));
        assert!(paths.contains(&("src/new.c".into(), FileState::New)));
        assert!(paths.contains(&("src/main.c".into(), FileState::Modified)));
        // The first commit's diff shows the files it added.
        let first = r.commit_diff(&h[1].id).unwrap();
        assert!(first.iter().all(|f| f.state == FileState::New));
    }

    #[test]
    fn line_changes_for_the_editor_margin() {
        let (_d, root) = project();
        let r = Repo::init(&root, "Ada", "ada@example.com").unwrap();
        let rel = Path::new("src/main.c");
        assert!(r.line_changes(rel, "int a;\nint b;\nint c;\n").is_empty());
        // Line 2 modified, a line added after line 3.
        let ch = r.line_changes(rel, "int a;\nint X;\nint c;\nint d;\n");
        assert_eq!(ch, vec![(2, LineChange::Modified), (4, LineChange::Added)]);
        // Line 2 deleted: marked on the line that follows.
        let ch = r.line_changes(rel, "int a;\nint c;\n");
        assert_eq!(ch, vec![(2, LineChange::DeletedAbove)]);
        // A file never committed: every line is new.
        let ch = r.line_changes(Path::new("src/other.c"), "x\ny\n");
        assert_eq!(ch, vec![(1, LineChange::Added), (2, LineChange::Added)]);
    }

    #[test]
    fn discard_restores_or_deletes() {
        let (_d, root) = project();
        let r = Repo::init(&root, "Ada", "ada@example.com").unwrap();
        std::fs::write(root.join("src/main.c"), "broken").unwrap();
        std::fs::write(root.join("src/tmp.c"), "temp").unwrap();
        r.discard(Path::new("src/main.c")).unwrap();
        r.discard(Path::new("src/tmp.c")).unwrap();
        assert_eq!(std::fs::read_to_string(root.join("src/main.c")).unwrap(), "int a;\nint b;\nint c;\n");
        assert!(!root.join("src/tmp.c").exists());
        assert!(r.status().unwrap().is_empty());
        assert_eq!(r.relative(&root.join("src/main.c")).unwrap(), Path::new("src/main.c"));
    }

    #[test]
    fn identity_is_required_and_stored_locally() {
        let (_d, root) = project();
        // A repository created by another tool, without user.name: GBADE must
        // ask instead of failing with a cryptic error.
        Repository::init(&root).unwrap();
        let r = Repo::open(&root).unwrap();
        if r.identity().is_none() {
            let e = r.commit_all("first").unwrap_err();
            assert!(e.message().contains("name and email"));
        }
        r.set_identity("Bo", "bo@example.com").unwrap();
        assert_eq!(r.identity(), Some(("Bo".into(), "bo@example.com".into())));
        r.commit_all("first").unwrap();
        assert_eq!(r.branch().is_empty(), false);
    }
}
