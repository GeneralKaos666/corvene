//! Port of GitHub Desktop's `app/test/unit/get-shell-env-test.ts`
//! (`lib/hooks/get-shell-env.ts`).
//!
//! GitHub Desktop's `getShellEnv(cwd, shellKind, printenvzPath)` (the
//! environment Git hooks run with, Settings › Git › Hooks) is Corvene's
//! `corvene_git::hook_env::load_shell_env()`; `{ kind: 'success', env }` is
//! `Ok(env)` and `{ kind: 'failure' }` an `Err`. On macOS and Linux both run
//! the user's login shell (GitHub Desktop's default shell; Corvene's
//! `$SHELL -ilc`) and collect the environment it prints. The test calls
//! `load_shell_env_with_timeout` with a longer timeout than the app's 5 s
//! so a busy machine does not fail it.
//!
//! On Windows GitHub Desktop runs the case once per
//! `SupportedHooksEnvShell` (`git-bash`, `pwsh`, `powershell`, `cmd`).
//! Corvene's Windows `load_shell_env` takes no shell kind and starts no
//! shell (it returns the process environment), so that variant calls the
//! stand-in `get_shell_env`.

// GHD: unit/get-shell-env-test.ts › getShellEnv › returns an env containing PATH (${label})
#[cfg(not(windows))]
#[test]
fn returns_an_env_containing_path_default_shell() {
    corvene_test_support::init();
    let result =
        corvene_git::hook_env::load_shell_env_with_timeout(std::time::Duration::from_secs(60));

    assert!(result.is_ok(), "expected kind 'success', got {result:?}");

    let Ok(env) = result else {
        return;
    };

    let path_key = env.keys().find(|k| k.to_lowercase() == "path");

    assert!(
        path_key.is_some(),
        "Expected env to contain a PATH key but got keys: {}",
        env.keys().cloned().collect::<Vec<_>>().join(", ")
    );
}

/// Stand-in for GitHub Desktop's `getShellEnv(undefined, shellKind)` with a
/// Windows `SupportedHooksEnvShell`. Replace it with the Corvene function
/// once there is one and remove the `#[ignore]`.
#[cfg(windows)]
fn get_shell_env(_shell_kind: &str) -> std::io::Result<std::collections::HashMap<String, String>> {
    unimplemented!("hook_env::load_shell_env takes no shell kind (lib/hooks/get-shell-env.ts)")
}

// GHD: unit/get-shell-env-test.ts › getShellEnv › returns an env containing PATH (${label})
#[cfg(windows)]
#[test]
#[ignore = "ghd: todo: Windows hook environment from a chosen shell (git-bash, pwsh, powershell, cmd; getShellEnv / getShell, Settings › Git › Hooks shell select)"]
fn returns_an_env_containing_path_windows_shells() {
    corvene_test_support::init();
    for shell_kind in ["git-bash", "pwsh", "powershell", "cmd"] {
        let result = get_shell_env(shell_kind);

        assert!(
            result.is_ok(),
            "expected kind 'success' ({shell_kind}), got {result:?}"
        );

        let Ok(env) = result else {
            continue;
        };

        let path_key = env.keys().find(|k| k.to_lowercase() == "path");

        assert!(
            path_key.is_some(),
            "Expected env to contain a PATH key but got keys: {}",
            env.keys().cloned().collect::<Vec<_>>().join(", ")
        );
    }
}
