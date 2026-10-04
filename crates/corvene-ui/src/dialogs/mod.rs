//! Dialog host: turns `AppState::popups` (GHD `PopupManager`) into live
//! dialog views. As in GHD (`app/src/ui/app.tsx` `renderPopups`), every
//! popup of the stack keeps its view, so one a newer popup covered comes
//! back as it was left, and only the top one (`isTopMost`) is shown. A
//! view is recreated only when its popup's value changes.

mod acknowledgements;
mod add_existing;
mod add_license;
mod app_dialogs;
pub(crate) mod branch_dialogs;
mod change_repository_alias;
mod ci_check_run_rerun;
pub(crate) mod clone_repository;
mod confirm_commit_to_default_branch;
mod confirm_delete_untrashable;
mod confirm_quit;
mod crash_report_found;
mod create_repository;
mod discard_changes;
mod discard_selection;
mod flags;
mod fork_dialogs;
mod history_dialogs;
mod ignore_with_pattern;
mod import_git_config;
mod import_github_desktop;
mod language_extensions;
mod mco_dialogs;
mod move_to_applications_folder;
mod move_to_shared_storage;
mod open_pull_request;
mod preferences;
mod pull_request_notifications;
mod push_branch_commits;
mod push_protection;
mod reauth_dialogs;
mod release_notes;
mod remote_dialogs;
mod remove_repositories;
mod repository_settings;
mod sign_in;
mod simple;
mod ssh_key_passphrase;
mod test_notifications;
mod tutorial_dialogs;
mod unknown_authors;
mod worktree_dialogs;

use corvene_core::{AppState, Popup};
use gpui_kit::prelude::*;
use gpui_kit::*;

pub use add_existing::AddExistingRepositoryDialog;
pub use app_dialogs::{AboutDialog, ConfirmRemoveRepositoryDialog, IntegrationErrorDialog};
#[doc(hidden)]
pub use branch_dialogs::sanitize_ref_name;
pub use branch_dialogs::{
    ConfirmOverwriteStashDialog, ConfirmSwitchBranchDialog, CreateBranchDialog, DeleteBranchDialog,
    MergeBranchDialog, RenameBranchDialog, StashAndSwitchBranchDialog,
};
pub use branch_dialogs::{
    StartPoint, get_start_point, render_branch_has_remote_warning,
    render_branch_name_exists_on_remote_warning,
};
pub use ci_check_run_rerun::CiCheckRunRerunDialog;
pub use clone_repository::CloneRepositoryDialog;
pub use confirm_commit_to_default_branch::ConfirmCommitToDefaultBranchDialog;
pub use create_repository::CreateRepositoryDialog;
pub use discard_changes::DiscardChangesDialog;
pub use discard_selection::DiscardSelectionDialog;
pub use flags::FlagsDialog;
pub use fork_dialogs::{
    ChooseForkSettingsDialog, CreateForkDialog, fork_settings_description,
    fork_settings_description_items, fork_settings_description_parts,
};
pub use history_dialogs::{
    CheckoutCommitDialog, ConfirmDeletePushedTagDialog, ConfirmDiscardStashDialog, CreateTagDialog,
    ResetToCommitDialog, ResetToRemoteDialog, UnreachableCommitsDialog,
    WarnLocalChangesBeforeUndoDialog, WarnTaggedCommitBeforeUndoDialog,
};
pub use mco_dialogs::{LocalChangesOverwrittenDialog, McoDialog, SquashCommitMessageDialog};
pub use open_pull_request::OpenPullRequestDialog;
pub use preferences::PreferencesDialog;
pub use pull_request_notifications::{
    PullRequestChecksFailedDialog, PullRequestCommentDialog, PullRequestReviewDialog,
};
pub use push_branch_commits::PushBranchCommitsDialog;
pub use push_protection::{BypassPushProtectionDialog, PushProtectionErrorDialog};
pub use reauth_dialogs::{
    InvalidatedTokenDialog, SamlReauthRequiredDialog, WorkflowPushRejectedDialog,
};
#[doc(hidden)]
pub use remote_dialogs::sanitized_repository_name;
pub use remote_dialogs::{
    ConfirmForcePushDialog, GenericGitAuthDialog, InitializeLfsDialog, PublishRepositoryDialog,
    PushNeedsPullDialog,
};
pub use repository_settings::{
    NoRemoteAction, NoRemoteContent, RepositorySettingsDialog, no_remote,
};
pub use sign_in::{
    ExistingAccountWarning, SignInAction, SignInContent, SignInDialog, sign_in_content,
};
pub use simple::{CliInstalledAction, CliInstalledContent, SimpleDialog, cli_installed};
pub use unknown_authors::UnknownAuthorsDialog;
pub use worktree_dialogs::{
    AddWorktreeDialog, DeleteWorktreeDialog, DeleteWorktreeFailedDialog, RenameWorktreeDialog,
};

pub struct DialogHost {
    state: Entity<AppState>,
    /// The view of each popup on the stack, by stack id.
    views: Vec<(u64, Popup, AnyView)>,
}

impl DialogHost {
    pub fn new(state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        Self {
            state,
            views: Vec::new(),
        }
    }

    fn build(&self, popup: &Popup, window: &mut Window, cx: &mut Context<Self>) -> AnyView {
        let state = self.state.clone();
        match popup {
            Popup::Error { .. }
            | Popup::IndexLockExists { .. }
            | Popup::InstallGit { .. }
            | Popup::CLIInstalled { .. } => cx.new(|_| SimpleDialog::new(popup.clone())).into(),
            Popup::AddExistingRepository { path } => cx
                .new(|cx| AddExistingRepositoryDialog::new(state, path.clone(), window, cx))
                .into(),
            Popup::CreateRepository { path } => cx
                .new(|cx| CreateRepositoryDialog::new(state, path.clone(), window, cx))
                .into(),
            Popup::CloneRepository { url } => cx
                .new(|cx| CloneRepositoryDialog::new(state, url.clone(), window, cx))
                .into(),
            Popup::CloneRepositoryRetry { url, path, error } => cx
                .new(|cx| {
                    CloneRepositoryDialog::retry(
                        state,
                        url.clone(),
                        path.clone(),
                        error.clone(),
                        window,
                        cx,
                    )
                })
                .into(),
            Popup::SignIn { enterprise } => cx
                .new(|cx| SignInDialog::new(state, *enterprise, window, cx))
                .into(),
            Popup::DiscardChanges { repo, paths, all } => cx
                .new(|_| DiscardChangesDialog::new(*repo, paths.clone(), *all))
                .into(),
            Popup::InvalidatedToken { account } => cx
                .new(|_| InvalidatedTokenDialog::new(account.clone()))
                .into(),
            Popup::StartPullRequest { repo } => cx
                .new(|cx| OpenPullRequestDialog::new(state, *repo, window, cx))
                .into(),
            Popup::CreateFork { repo } => {
                cx.new(|cx| CreateForkDialog::new(state, *repo, cx)).into()
            }
            Popup::UpstreamAlreadyExists { repo, existing_url } => cx
                .new(|_| {
                    fork_dialogs::UpstreamAlreadyExistsDialog::new(
                        state,
                        *repo,
                        existing_url.clone(),
                    )
                })
                .into(),
            Popup::ChooseForkSettings { repo } => cx
                .new(|cx| ChooseForkSettingsDialog::new(state, *repo, cx))
                .into(),
            Popup::PushProtectionError {
                repo,
                secrets,
                bypassed,
            } => cx
                .new(|_| PushProtectionErrorDialog::new(*repo, secrets.clone(), bypassed.clone()))
                .into(),
            Popup::BypassPushProtection {
                repo,
                secret,
                secrets,
                bypassed,
            } => cx
                .new(|_| {
                    BypassPushProtectionDialog::new(
                        *repo,
                        secret.clone(),
                        secrets.clone(),
                        bypassed.clone(),
                    )
                })
                .into(),
            Popup::PushRejectedDueToMissingWorkflowScope {
                repo,
                rejected_path,
            } => cx
                .new(|_| WorkflowPushRejectedDialog::new(*repo, rejected_path.clone()))
                .into(),
            Popup::SAMLReauthRequired {
                repo,
                organization,
                endpoint,
                retry,
            } => cx
                .new(|_| {
                    SamlReauthRequiredDialog::new(
                        *repo,
                        organization.clone(),
                        endpoint.clone(),
                        retry.clone(),
                    )
                })
                .into(),
            Popup::CICheckRunRerun {
                github,
                checks,
                git_ref,
                failed_only,
                ..
            } => cx
                .new(|cx| {
                    CiCheckRunRerunDialog::new(
                        github.clone(),
                        checks.clone(),
                        git_ref.clone(),
                        *failed_only,
                        cx,
                    )
                })
                .into(),
            Popup::PullRequestReview {
                repo,
                pull_request,
                review,
                should_checkout_branch,
                should_change_repository,
            } => cx
                .new(|cx| {
                    PullRequestReviewDialog::new(
                        *repo,
                        pull_request.clone(),
                        review.clone(),
                        *should_checkout_branch,
                        *should_change_repository,
                        cx,
                    )
                })
                .into(),
            Popup::PullRequestComment {
                repo,
                pull_request,
                comment,
                should_checkout_branch,
                should_change_repository,
            } => cx
                .new(|cx| {
                    PullRequestCommentDialog::new(
                        *repo,
                        pull_request.clone(),
                        comment.clone(),
                        *should_checkout_branch,
                        *should_change_repository,
                        cx,
                    )
                })
                .into(),
            Popup::PullRequestChecksFailed {
                repo,
                pull_request,
                checks,
                should_change_repository,
            } => cx
                .new(|_| {
                    PullRequestChecksFailedDialog::new(
                        *repo,
                        pull_request.clone(),
                        checks.clone(),
                        *should_change_repository,
                    )
                })
                .into(),
            Popup::UnknownAuthors {
                repo,
                usernames,
                summary,
                description,
            } => cx
                .new(|_| {
                    UnknownAuthorsDialog::new(
                        *repo,
                        usernames.clone(),
                        summary.clone(),
                        description.clone(),
                    )
                })
                .into(),
            Popup::ConfirmCommitToDefaultBranch {
                repo,
                branch,
                summary,
                description,
                unknown_co_authors,
            } => cx
                .new(|_| {
                    ConfirmCommitToDefaultBranchDialog::new(
                        *repo,
                        branch.clone(),
                        summary.clone(),
                        description.clone(),
                        unknown_co_authors.clone(),
                    )
                })
                .into(),
            Popup::ConfirmDiscardSelection {
                repo,
                path,
                selection,
            } => cx
                .new(|_| DiscardSelectionDialog::new(*repo, path.clone(), selection.clone()))
                .into(),
            Popup::ResetToCommit { repo, sha } => cx
                .new(|_| ResetToCommitDialog::new(*repo, sha.clone()))
                .into(),
            Popup::ResetToRemote {
                repo,
                branch,
                upstream,
                ahead,
                dirty,
                commit,
                files,
            } => cx
                .new(|_| {
                    ResetToRemoteDialog::new(
                        *repo,
                        branch.clone(),
                        upstream.clone(),
                        *ahead,
                        *dirty,
                    )
                    .for_commit(commit.clone(), files.clone())
                })
                .into(),
            Popup::CheckoutCommit { repo, sha } => cx
                .new(|_| CheckoutCommitDialog::new(*repo, sha.clone()))
                .into(),
            Popup::CreateTag { repo, sha } => cx
                .new(|cx| CreateTagDialog::new(*repo, sha.clone(), window, cx))
                .into(),
            Popup::WarnLocalChangesBeforeUndo { repo } => cx
                .new(|_| WarnLocalChangesBeforeUndoDialog::new(*repo))
                .into(),
            Popup::ConfirmDeletePushedTag { repo, tag } => cx
                .new(|cx| ConfirmDeletePushedTagDialog::new(*repo, tag.clone(), cx))
                .into(),
            Popup::WarnTaggedCommitBeforeUndo {
                repo,
                tags,
                warn_local,
            } => cx
                .new(|_| WarnTaggedCommitBeforeUndoDialog::new(*repo, tags.clone(), *warn_local))
                .into(),
            Popup::WarnTaggedCommitBeforeAmend { repo, sha, tags } => cx
                .new(|_| WarnTaggedCommitBeforeUndoDialog::amend(*repo, tags.clone(), sha.clone()))
                .into(),
            Popup::CreateBranch {
                repo,
                target_sha,
                initial_name,
            } => cx
                .new(|cx| {
                    CreateBranchDialog::new(
                        state,
                        *repo,
                        target_sha.clone(),
                        initial_name.clone(),
                        window,
                        cx,
                    )
                })
                .into(),
            Popup::RenameBranch { repo, name } => cx
                .new(|cx| RenameBranchDialog::new(state, *repo, name.clone(), window, cx))
                .into(),
            Popup::ChangeRepositoryAlias { repo } => cx
                .new(|cx| {
                    change_repository_alias::ChangeRepositoryAliasDialog::new(
                        state, *repo, window, cx,
                    )
                })
                .into(),
            Popup::AddWorktree {
                repo,
                initial_branch_name,
                initial_worktree_name,
            } => cx
                .new(|cx| {
                    AddWorktreeDialog::new(
                        state,
                        *repo,
                        initial_branch_name.clone(),
                        initial_worktree_name.clone(),
                        window,
                        cx,
                    )
                })
                .into(),
            Popup::RenameWorktree { repo, path } => cx
                .new(|cx| RenameWorktreeDialog::new(*repo, path.clone(), window, cx))
                .into(),
            Popup::DeleteWorktree { repo, path } => cx
                .new(|_| DeleteWorktreeDialog::new(*repo, path.clone()))
                .into(),
            Popup::DeleteWorktreeFailed {
                repo,
                path,
                error,
                original,
            } => cx
                .new(|_| {
                    DeleteWorktreeFailedDialog::new(
                        *repo,
                        path.clone(),
                        error.clone(),
                        original.clone(),
                    )
                })
                .into(),
            Popup::ConfirmSwitchBranch { repo, branch } => cx
                .new(|_| ConfirmSwitchBranchDialog::new(*repo, branch.clone()))
                .into(),
            Popup::DeleteBranch { repo, name } => cx
                .new(|cx| DeleteBranchDialog::new(state, *repo, name.clone(), cx))
                .into(),
            Popup::StashAndSwitchBranch { repo, branch } => cx
                .new(|_| StashAndSwitchBranchDialog::new(state, *repo, branch.clone()))
                .into(),
            Popup::ConfirmOverwriteStash { repo, branch } => cx
                .new(|_| ConfirmOverwriteStashDialog::new(*repo, branch.clone()))
                .into(),
            Popup::MergeBranch { repo, squash } => cx
                .new(|cx| MergeBranchDialog::new(state, *repo, *squash, window, cx))
                .into(),
            Popup::ConfirmDiscardStash { repo } => {
                cx.new(|_| ConfirmDiscardStashDialog::new(*repo)).into()
            }
            Popup::PublishRepository { repo } => cx
                .new(|cx| PublishRepositoryDialog::new(state, *repo, window, cx))
                .into(),
            Popup::PushNeedsPull { repo } => cx.new(|_| PushNeedsPullDialog::new(*repo)).into(),
            Popup::PushBranchCommits {
                repo,
                branch,
                unpushed,
                base,
            } => cx
                .new(|_| {
                    PushBranchCommitsDialog::new(*repo, branch.clone(), *unpushed, base.clone())
                })
                .into(),
            Popup::ConfirmForcePush {
                repo,
                upstream_branch,
            } => cx
                .new(|_| ConfirmForcePushDialog::new(*repo, upstream_branch.clone()))
                .into(),
            Popup::GenericGitAuthentication {
                repo,
                remote_url,
                host,
                username,
                retry,
            } => cx
                .new(|cx| {
                    GenericGitAuthDialog::new(
                        *repo,
                        remote_url.clone(),
                        host.clone(),
                        username.clone(),
                        retry.clone(),
                        window,
                        cx,
                    )
                })
                .into(),
            Popup::InitializeLFS { repos } => cx
                .new(|_| InitializeLfsDialog::new(state, repos.clone()))
                .into(),
            Popup::MultiCommitOperation { repo, .. } => {
                cx.new(|cx| McoDialog::new(state, *repo, window, cx)).into()
            }
            Popup::LocalChangesOverwritten { repo, retry, files } => cx
                .new(|_| {
                    LocalChangesOverwrittenDialog::new(state, *repo, retry.clone(), files.clone())
                })
                .into(),
            Popup::SquashCommitMessage {
                repo,
                to_squash,
                onto,
                summary,
                description,
                count,
            } => {
                // flag `827`: the target commit's own description, for "Keep
                // Target's Message"
                let target_body = {
                    let s = state.read(cx);
                    s.repo_states
                        .get(repo)
                        .and_then(|r| r.commits.iter().find(|c| c.sha == *onto))
                        .map(|c| c.body.trim().to_string())
                        .filter(|_| {
                            // not when editing one message (`892`)
                            !to_squash.is_empty()
                                && s.flags
                                    .bool(corvene_core::flags::ids::SQUASH_KEEP_TARGET_MESSAGE)
                        })
                };
                cx.new(|cx| {
                    SquashCommitMessageDialog::new(
                        *repo,
                        to_squash.clone(),
                        onto.clone(),
                        summary.clone(),
                        description.clone(),
                        target_body,
                        *count,
                        window,
                        cx,
                    )
                })
                .into()
            }
            Popup::Preferences { tab } => cx
                .new(|cx| PreferencesDialog::new(state, *tab, window, cx))
                .into(),
            Popup::Flags { query } => cx
                .new(|cx| FlagsDialog::new(state, query.clone(), window, cx))
                .into(),
            Popup::LanguageExtensions { focus, .. } => cx
                .new(|cx| {
                    language_extensions::LanguageExtensionsDialog::new(
                        state,
                        focus.clone(),
                        window,
                        cx,
                    )
                })
                .into(),
            Popup::RepositorySettings { repo, tab } => cx
                .new(|cx| RepositorySettingsDialog::new(state, *repo, *tab, window, cx))
                .into(),
            Popup::ConfirmRemoveRepository { repo } => cx
                .new(|_| ConfirmRemoveRepositoryDialog::new(state, *repo))
                .into(),
            Popup::About { version } => cx
                .new(|cx| AboutDialog::new(state, version.clone(), cx))
                .into(),
            Popup::MoveToApplicationsFolder => cx
                .new(|_| move_to_applications_folder::MoveToApplicationsFolderDialog::new())
                .into(),
            Popup::Acknowledgements => cx.new(acknowledgements::AcknowledgementsDialog::new).into(),
            Popup::ImportFromGitHubDesktop => cx
                .new(import_github_desktop::ImportGitHubDesktopDialog::new)
                .into(),
            Popup::RemoveRepositories { ticked } => cx
                .new(|cx| {
                    remove_repositories::RemoveRepositoriesDialog::new(state, *ticked, window, cx)
                })
                .into(),
            Popup::ConfirmDeleteUntrashable { repo, paths } => cx
                .new(|_| {
                    confirm_delete_untrashable::ConfirmDeleteUntrashableDialog::new(
                        *repo,
                        paths.clone(),
                    )
                })
                .into(),
            Popup::IgnoreWithPattern { repo, pattern } => cx
                .new(|cx| {
                    ignore_with_pattern::IgnoreWithPatternDialog::new(
                        *repo,
                        pattern.clone(),
                        window,
                        cx,
                    )
                })
                .into(),
            Popup::AddLicense { repo } => {
                cx.new(|_| add_license::AddLicenseDialog::new(*repo)).into()
            }
            Popup::CreateTutorialRepository { account, progress } => cx
                .new(|_| {
                    tutorial_dialogs::CreateTutorialRepositoryDialog::new(
                        account.clone(),
                        progress.clone(),
                    )
                })
                .into(),
            Popup::ConfirmExitTutorial => cx
                .new(|_| tutorial_dialogs::ConfirmExitTutorialDialog)
                .into(),
            Popup::ImportGitConfig { settings, skipped } => cx
                .new(|_| import_git_config::ImportGitConfigDialog::new(settings.clone(), *skipped))
                .into(),
            Popup::SshKeyPassphrase { path, wrong } => cx
                .new(|cx| {
                    ssh_key_passphrase::SshKeyPassphraseDialog::new(
                        path.clone(),
                        *wrong,
                        window,
                        cx,
                    )
                })
                .into(),
            Popup::TestNotifications { repo } => cx
                .new(|cx| test_notifications::TestNotificationsDialog::new(*repo, cx))
                .into(),
            Popup::CrashReportFound { reports } => cx
                .new(|_| crash_report_found::CrashReportFoundDialog::new(reports.clone()))
                .into(),
            Popup::ReleaseNotes { summary } => cx
                .new(|cx| release_notes::ReleaseNotesDialog::new(state, summary.clone(), cx))
                .into(),
            Popup::ExternalEditorError { .. } | Popup::ShellError { .. } => cx
                .new(|_| IntegrationErrorDialog::new(popup.clone()))
                .into(),
            Popup::MoveToSharedStorage { repo, then } => cx
                .new(|cx| {
                    move_to_shared_storage::MoveToSharedStorageDialog::new(
                        state,
                        *repo,
                        then.clone(),
                        window,
                        cx,
                    )
                })
                .into(),
            Popup::UnreachableCommits { repo, tab } => cx
                .new(|_| UnreachableCommitsDialog::new(state, *repo, *tab))
                .into(),
            Popup::ConfirmQuit { busy } => cx
                .new(|_| confirm_quit::ConfirmQuitDialog::new(busy))
                .into(),
        }
    }
}

impl Render for DialogHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let stack: Vec<(u64, Popup)> = self
            .state
            .read(cx)
            .popups
            .all_popups()
            .iter()
            .filter_map(|p| Some((p.id?, p.popup.clone())))
            .collect();
        // views of popups that left the stack go
        self.views
            .retain(|(id, _, _)| stack.iter().any(|(open, _)| open == id));
        // the top popup's view, built (again) when new or changed
        let Some((id, popup)) = stack.last().cloned() else {
            return div();
        };
        match self.views.iter().position(|(open, _, _)| *open == id) {
            Some(ix) if self.views[ix].1 == popup => {}
            Some(ix) => {
                let view = self.build(&popup, window, cx);
                self.views[ix] = (id, popup, view);
            }
            None => {
                let view = self.build(&popup, window, cx);
                self.views.push((id, popup, view));
            }
        }
        let view = self
            .views
            .iter()
            .find(|(open, _, _)| *open == id)
            .map(|(_, _, view)| view.clone());
        div().children(view)
    }
}
