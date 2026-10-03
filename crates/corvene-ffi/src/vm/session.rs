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
