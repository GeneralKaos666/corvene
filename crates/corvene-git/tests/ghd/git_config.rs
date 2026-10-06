//! Port of GitHub Desktop's `app/test/unit/git/config-test.ts`.
//!
//! Corvene equivalents (`lib/git/config.ts` → `corvene_git::config`):
//!
//! - `getConfigValue(repository, name)` (not only local) is
//!   `corvene_git::config_value(git, path, name)` (`git config --get`);
//!   `None` is GitHub Desktop's `null`.
//! - GitHub Desktop's `git(args, path, name, options)` (`lib/git/core.ts`)
//!   is `corvene_git::GitCommand`: `env` is `.env(..)`,
//!   `successExitCodes: new Set([1])` is `.allow_exit_code(1)` plus a check
//!   that git exited with 1 (`GitCommand` keeps 0 a success too).
//! - `getGlobalConfigValue(name, env)` is
//!   `corvene_git::global_config_value(git, name)` and
//!   `setGlobalConfigValue(name, value, env)`
//!   `corvene_git::set_global_config_value(git, name, value)`.
//! - `getGlobalConfigPath(env)` is `corvene_git::global_config_path(git)`
//!   (Settings › Git's "edit your global Git config file",
//!   `Dispatcher::edit_global_git_config`) and
//!   `getGlobalBooleanConfigValue(name, env)`
//!   `corvene_git::global_boolean_config_value(git, name)`.
//!
//! The `global config` cases give each call `env: { HOME }`, a new
//! temporary home, so the global configuration is private to the test.
//! Corvene's global config functions take no environment; they read and
//! write the global configuration of the test process, whose `HOME` is
//! already an empty temporary directory, so [`setup`] holds
//! `lock_global_config` (an empty, private `$HOME/.gitconfig` for the whole
//! test) instead.

use std::path::PathBuf;

use corvene_test_support::{
    GlobalConfigGuard, exec, git, git_command, home_dir, lock_global_config,
    setup_fixture_repository,
};

/// `getGlobalConfigPath(env)`.
fn get_global_config_path() -> PathBuf {
    corvene_git::global_config_path(git()).expect("getGlobalConfigPath")
}

/// `getGlobalBooleanConfigValue(name, env)`.
fn get_global_boolean_config_value(name: &str) -> Option<bool> {
    corvene_git::global_boolean_config_value(git(), name)
}

/// The `global config` describe's `setup(t)`: the private global
/// configuration (see the module docs), its path and `git config -f
/// <path>`.
struct GlobalConfigSetup {
    _guard: GlobalConfigGuard,
    expected_config_path: PathBuf,
    base_args: Vec<String>,
}

fn setup() -> GlobalConfigSetup {
    let guard = lock_global_config();
    let expected_config_path = guard.path();
    let base_args = vec![
        "config".to_string(),
        "-f".to_string(),
        expected_config_path.to_string_lossy().into_owned(),
    ];
    GlobalConfigSetup {
        _guard: guard,
        expected_config_path,
        base_args,
    }
}

/// `exec([...baseArgs, ...args], __dirname)`.
fn exec_with_base_args(base_args: &[String], args: &[&str]) {
    let mut all: Vec<&str> = base_args.iter().map(String::as_str).collect();
    all.extend_from_slice(args);
    exec(all, home_dir());
}

// GHD: unit/git/config-test.ts › git/config › config › looks up config values
#[test]
fn looks_up_config_values() {
    let repository = setup_fixture_repository("test-repo");
    let bare = corvene_git::config_value(git(), repository.path(), "core.bare");
    assert_eq!(bare.as_deref(), Some("false"));
}

// GHD: unit/git/config-test.ts › git/config › config › returns null for undefined values
#[test]
fn returns_null_for_undefined_values() {
    let repository = setup_fixture_repository("test-repo");
    let value = corvene_git::config_value(git(), repository.path(), "core.the-meaning-of-life");
    assert!(value.is_none(), "{value:?}");
}

// GHD: unit/git/config-test.ts › git/config › GIT_CONFIG_PARAMETERS › picks them up
#[test]
fn picks_them_up() {
    let repository = setup_fixture_repository("test-repo");

    let without_env = git_command(repository.path())
        .args(["config", "desktop.test"])
        .allow_exit_code(1)
        .run()
        .expect("git config desktop.test");
    // `successExitCodes: new Set([1])`
    assert_eq!(without_env.status.code(), Some(1));
    let without_env_output = without_env.stdout;

    assert_eq!(
        without_env_output.len(),
        0,
        "Expected withoutEnvOutput to be empty"
    );
    let with_env_output = git_command(repository.path())
        .args(["config", "desktop.test"])
        .env("GIT_CONFIG_PARAMETERS", "'desktop.test=1'")
        .run()
        .expect("git config desktop.test")
        .stdout;

    assert_eq!(String::from_utf8(with_env_output).unwrap(), "1\n");
}

// GHD: unit/git/config-test.ts › git/config › GIT_CONFIG_PARAMETERS › takes precedence over GIT_CONFIG_*
#[test]
fn takes_precedence_over_git_config() {
    let repository = setup_fixture_repository("test-repo");

    let output = git_command(repository.path())
        .args(["config", "user.name"])
        .env("GIT_CONFIG_PARAMETERS", "'user.name=foobar'")
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "user.name")
        .env("GIT_CONFIG_VALUE_0", "baz")
        .run()
        .expect("git config user.name")
        .stdout;

    assert_eq!(String::from_utf8(output).unwrap(), "foobar\n");
}

// GHD: unit/git/config-test.ts › git/config › global config › getGlobalConfigPath › gets the config path
#[test]
fn gets_the_config_path() {
    let setup = setup();

    // getGlobalConfigPath requires at least one entry, so the
    // test needs to setup an existing config value
    exec_with_base_args(&setup.base_args, &["user.name", "bar"]);

    let path = get_global_config_path();
    assert_eq!(
        path,
        dunce::canonicalize(&setup.expected_config_path).unwrap()
    );
}

// GHD: unit/git/config-test.ts › git/config › global config › setGlobalConfigValue › will replace all entries for a global value
#[test]
fn will_replace_all_entries_for_a_global_value() {
    let setup = setup();
    let key = "foo.bar";

    exec_with_base_args(&setup.base_args, &["--add", key, "first"]);
    exec_with_base_args(&setup.base_args, &["--add", key, "second"]);

    corvene_git::set_global_config_value(git(), key, "the correct value")
        .expect("setGlobalConfigValue");
    let value = corvene_git::global_config_value(git(), key);
    assert_eq!(value.as_deref(), Some("the correct value"));
}

/// The `getGlobalBooleanConfigValue` cases: set `foo.bar` to `value`, then
/// read it back as a boolean.
fn global_boolean_config_value_of(value: &str) -> Option<bool> {
    let key = "foo.bar";
    let _setup = setup();

    corvene_git::set_global_config_value(git(), key, value).expect("setGlobalConfigValue");
    get_global_boolean_config_value(key)
}

// GHD: unit/git/config-test.ts › git/config › global config › getGlobalBooleanConfigValue › treats "false" as false
#[test]
fn treats_false_as_false() {
    let value = global_boolean_config_value_of("false");
    assert_eq!(value, Some(false));
}

// GHD: unit/git/config-test.ts › git/config › global config › getGlobalBooleanConfigValue › treats "off" as false
#[test]
fn treats_off_as_false() {
    let value = global_boolean_config_value_of("off");
    assert_eq!(value, Some(false));
}

// GHD: unit/git/config-test.ts › git/config › global config › getGlobalBooleanConfigValue › treats "no" as false
#[test]
fn treats_no_as_false() {
    let value = global_boolean_config_value_of("no");
    assert_eq!(value, Some(false));
}

// GHD: unit/git/config-test.ts › git/config › global config › getGlobalBooleanConfigValue › treats "0" as false
#[test]
fn treats_0_as_false() {
    let value = global_boolean_config_value_of("0");
    assert_eq!(value, Some(false));
}

// GHD: unit/git/config-test.ts › git/config › global config › getGlobalBooleanConfigValue › treats "true" as true
#[test]
fn treats_true_as_true() {
    let value = global_boolean_config_value_of("true");
    assert_eq!(value, Some(true));
}

// GHD: unit/git/config-test.ts › git/config › global config › getGlobalBooleanConfigValue › treats "yes" as true
#[test]
fn treats_yes_as_true() {
    let value = global_boolean_config_value_of("yes");
    assert_eq!(value, Some(true));
}

// GHD: unit/git/config-test.ts › git/config › global config › getGlobalBooleanConfigValue › treats "on" as true
#[test]
fn treats_on_as_true() {
    let value = global_boolean_config_value_of("on");
    assert_eq!(value, Some(true));
}

// GHD: unit/git/config-test.ts › git/config › global config › getGlobalBooleanConfigValue › treats "1" as true
#[test]
fn treats_1_as_true() {
    let value = global_boolean_config_value_of("1");
    assert_eq!(value, Some(true));
}
