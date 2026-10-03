//! GitHub Desktop 3.6.6 unit tests ported to Corvene. Each module is one
//! GitHub Desktop test file (`app/test/unit/...`); `*_support` modules hold
//! helpers shared inside this crate. See `tools/ghd-tests/README.md`.

mod accounts_store;
mod accounts_support;
mod ahead_behind_store;
mod branch_pruner;
mod ci_checks;
mod clone_path_safety;
mod cloning_repositories_store;
mod cloning_repository;
mod cloning_support;
mod filter_changes_logic;
mod find_account;
mod find_forked_remotes_to_prune;
mod find_upstream_remote;
mod fuzzy_find;
mod get_file_hash;
mod git_git_attributes;
mod git_store_cache;
mod list_selection;
mod main_process_proxy;
mod multi_commit_operation;
mod offset_from;
mod parse_app_url;
mod path;
mod popup_manager;
mod remote_parsing;
mod repo_rules;
mod repositories_store;
mod repository_state_cache;
mod round;
mod sign_in_store;
mod squashed_commit_description;
mod stores_github_user_store;
mod stores_support;
mod stores_updates_update_changed_files;
mod stores_updates_update_conflict_state;
mod text_token_parser;
mod text_tokens_support;
mod truncate_with_ellipsis;
mod unique_coauthors_as_authors;
mod welcome;
mod wrap_rich_text_commit_message;
