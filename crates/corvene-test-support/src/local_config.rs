//! Port of `app/test/helpers/local-config.ts`.

use crate::exec::exec;
use crate::repositories::TestRepo;

/// GitHub Desktop's `setupLocalConfig(repository, localConfig)`: `git config
/// <key> <value>` in the repository for each pair. Like GitHub Desktop it
/// does not check the exit codes.
pub fn setup_local_config<I, K, V>(repository: &TestRepo, local_config: I)
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    for (key, value) in local_config {
        exec(["config", key.as_ref(), value.as_ref()], repository.path());
    }
}
