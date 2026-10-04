//! Sign-in, the accounts, and the clone in progress: what the Welcome
//! flow, the Sign-in screen and the Clone dialog show.

use corvene_core::AppState;
use corvene_core::state::SignInStep;

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct AccountVm {
    /// API base, e.g. `https://api.github.com`.
    pub endpoint: String,
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
    pub host: String,
}

#[derive(uniffi::Enum, Clone, Debug, PartialEq, Eq)]
pub enum SignInStepVm {
    Requesting,
    /// Show the code; GitHub polls until the browser authorised it.
    DeviceCode {
        user_code: String,
        verification_uri: String,
    },
    Verifying,
    /// The browser flow: the authorize page is (or should be) open.
    Browser {
        authorize_url: String,
    },
    Error {
        message: String,
    },
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct SignInVm {
    pub endpoint: String,
    pub step: SignInStepVm,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct CloneProgressVm {
    pub url: String,
    pub path: String,
    pub description: String,
    /// 0..1; `None` = indeterminate.
    pub value: Option<f32>,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq)]
pub struct SessionVm {
    pub accounts: Vec<AccountVm>,
    pub sign_in: Option<SignInVm>,
    pub cloning: Option<CloneProgressVm>,
    pub welcome_completed: bool,
}

pub fn session(s: &AppState) -> SessionVm {
    SessionVm {
        accounts: s
            .accounts
            .iter()
            .map(|a| AccountVm {
                endpoint: a.endpoint.clone(),
                login: a.login.clone(),
                name: a.name.clone(),
                avatar_url: a.avatar_url.clone(),
                host: a.host(),
            })
            .collect(),
        sign_in: s.sign_in.as_ref().map(|si| SignInVm {
            endpoint: si.endpoint.clone(),
            step: match &si.step {
                SignInStep::Requesting => SignInStepVm::Requesting,
                SignInStep::DeviceCode {
                    user_code,
                    verification_uri,
                } => SignInStepVm::DeviceCode {
                    user_code: user_code.clone(),
                    verification_uri: verification_uri.clone(),
                },
                SignInStep::Verifying => SignInStepVm::Verifying,
                SignInStep::Browser { authorize_url } => SignInStepVm::Browser {
                    authorize_url: authorize_url.clone(),
                },
                SignInStep::Error(message) => SignInStepVm::Error {
                    message: message.clone(),
                },
            },
        }),
        cloning: s.cloning.as_ref().map(|c| CloneProgressVm {
            url: c.url.clone(),
            path: c.path.to_string_lossy().into_owned(),
            description: c.description.clone(),
            value: c.value,
        }),
        welcome_completed: s.settings.welcome_completed,
    }
}

/// A repository of the signed-in account, for the Clone dialog's list.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct CloneableRepositoryVm {
    pub owner: String,
    pub name: String,
    pub clone_url: String,
    pub html_url: String,
    pub private: bool,
    pub fork: bool,
    pub default_branch: Option<String>,
}

#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct CloneableRepositoriesVm {
    pub endpoint: String,
    pub loading: bool,
    /// `None` until loaded.
    pub repositories: Option<Vec<CloneableRepositoryVm>>,
}

pub fn cloneable_repositories(s: &AppState, endpoint: &str) -> CloneableRepositoriesVm {
    CloneableRepositoriesVm {
        endpoint: endpoint.to_string(),
        loading: s.api_repositories_loading.contains(endpoint),
        repositories: s.api_repositories.get(endpoint).map(|repos| {
            repos
                .iter()
                .map(|r| CloneableRepositoryVm {
                    owner: r.owner.clone(),
                    name: r.name.clone(),
                    clone_url: r.clone_url.clone(),
                    html_url: r.html_url.clone(),
                    private: r.private,
                    fork: r.fork,
                    default_branch: r.default_branch.clone(),
                })
                .collect()
        }),
    }
}
