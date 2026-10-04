//! GitHub Desktop's model constructors and accessors that Corvene's models
//! (`corvene_models`) lack, as GitHub Desktop's tests use them.

use corvene_models::{
    DiffHunk, DiffLine, DiffLineKind, DiffSelection, FileStatus, FileStatusKind, GitStatusEntry,
    WorkingDirectoryFileChange, WorkingDirectoryStatus,
};

/// GitHub Desktop's `new WorkingDirectoryFileChange(path, { kind },
/// selection)` (`models/status.ts`). GitHub Desktop's `{ kind }` status has
/// nothing but the kind; Corvene's `FileStatus` also has the porcelain
/// columns, which are left neutral here: index and working tree
/// `Unchanged`, no code, no score, not a submodule, no conflict markers.
/// Set a column on the result when the code under test reads it.
pub fn working_directory_file_change(
    path: &str,
    kind: FileStatusKind,
    selection: DiffSelection,
) -> WorkingDirectoryFileChange {
    WorkingDirectoryFileChange {
        path: path.to_string(),
        old_path: None,
        status: FileStatus {
            kind,
            index: GitStatusEntry::Unchanged,
            working_tree: GitStatusEntry::Unchanged,
            score: None,
            code: String::new(),
            submodule: false,
            submodule_status: None,
            conflict_markers: None,
        },
        selection,
    }
}

/// GitHub Desktop's `WorkingDirectoryStatus.fromFiles(files)`
/// (`models/status.ts`): a status of these files and nothing else.
pub fn from_files(files: Vec<WorkingDirectoryFileChange>) -> WorkingDirectoryStatus {
    WorkingDirectoryStatus {
        files,
        ..Default::default()
    }
}

/// What [`conflicted_count`] counts in: a working directory status or its
/// files.
pub trait FileChanges {
    fn file_changes(&self) -> &[WorkingDirectoryFileChange];
}

impl FileChanges for [WorkingDirectoryFileChange] {
    fn file_changes(&self) -> &[WorkingDirectoryFileChange] {
        self
    }
}

impl FileChanges for Vec<WorkingDirectoryFileChange> {
    fn file_changes(&self) -> &[WorkingDirectoryFileChange] {
        self
    }
}

impl FileChanges for WorkingDirectoryStatus {
    fn file_changes(&self) -> &[WorkingDirectoryFileChange] {
        &self.files
    }
}

/// The number of conflicted files: GitHub Desktop's tests'
/// `files.filter(f => f.status.kind === AppFileStatusKind.Conflicted).length`.
pub fn conflicted_count<F: FileChanges + ?Sized>(files: &F) -> usize {
    files
        .file_changes()
        .iter()
        .filter(|f| f.status.kind == FileStatusKind::Conflicted)
        .count()
}

/// GitHub Desktop's `DiffLine.text` (`models/diff/diff-line.ts`) of a
/// Corvene line: GitHub Desktop keeps the `+` / `-` / space marker in the
/// text, Corvene keeps it in `DiffLine::kind`. A hunk header is its text.
pub fn ghd_text(line: &DiffLine) -> String {
    match line.kind {
        DiffLineKind::Hunk => line.text.clone(),
        DiffLineKind::Add => format!("+{}", line.text),
        DiffLineKind::Delete => format!("-{}", line.text),
        DiffLineKind::Context => format!(" {}", line.text),
    }
}

/// GitHub Desktop's `DiffHunk.unifiedDiffEnd` (`models/diff/raw-diff.ts`):
/// the index of the hunk's last line in the unified diff
/// (`linesConsumed + lines.length - 1` in `DiffParser.parseHunk`; `lines`
/// starts with the header line, as Corvene's do). Corvene's `DiffHunk` keeps
/// only the start.
pub fn unified_diff_end(hunk: &DiffHunk) -> u32 {
    hunk.unified_diff_start + hunk.lines.len() as u32 - 1
}
