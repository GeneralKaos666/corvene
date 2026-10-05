//! Sign in to GitLab, Gitea / Forgejo or Bitbucket (Corvene addition, flags
//! `342-gitlab`, `343-gitea`, `344-bitbucket`; `corvene_core::hosts`). GHD
//! signs in to GitHub only. Laid out like its Enterprise sign-in: a server
//! address (Bitbucket Cloud has one), then a token, or the browser when an
//! OAuth application is known (built into the build or its ID typed here).

use corvene_core::{AppState, Dispatcher, HostEndpoint, HostKind};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, dialog};
use crate::icons::{Octicon, octicon};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{button, labeled, password_text_box, text_box};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Method {
    Token,
    Browser,
}

pub struct SignInHostDialog {
    state: Entity<AppState>,
    kind: HostKind,
    method: Method,
    address: Entity<InputState>,
    token: Entity<InputState>,
    email: Entity<InputState>,
    client_id: Entity<InputState>,
}

/// The dialog's title.
pub fn title(kind: HostKind) -> &'static str {
    match kind {
        HostKind::GitHub => "Sign in to GitHub.com",
        HostKind::GitLab => "Sign in to GitLab",
        HostKind::Gitea => "Sign in to Gitea or Forgejo",
        HostKind::Bitbucket => "Sign in to Bitbucket",
    }
}

/// What the token paragraph asks for.
pub fn token_help(kind: HostKind) -> &'static str {
    match kind {
        HostKind::GitHub => {
            "Create a token with the repo, workflow, read:user and user:email scopes."
        }
        HostKind::GitLab => {
            "Create a personal access token with the api, read_user and write_repository \
             scopes, then paste it here."
        }
        HostKind::Gitea => {
            "Create an access token with read and write access to repositories and read \
             access to the user, then paste it here."
        }
        HostKind::Bitbucket => {
            "Create an Atlassian API token for Bitbucket with the read:user, \
             read:repository, write:repository, read:pullrequest and write:pullrequest \
             scopes, then enter it with your Atlassian account's email."
        }
    }
}

impl SignInHostDialog {
    pub fn new(
        state: Entity<AppState>,
        kind: HostKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        let default = HostEndpoint::public(kind);
        let address = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder(default.web_base.clone());
            input.set_value(default.web_base.clone(), window, cx);
            input
        });
        let token = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(match kind {
                    HostKind::GitLab => "glpat-…",
                    HostKind::Bitbucket => "ATATT…",
                    _ => "",
                })
                .masked(true)
        });
        let email = cx.new(|cx| InputState::new(window, cx).placeholder("you@example.com"));
        let built_in = corvene_hosts::oauth::OAuthApp::built_in(&default)
            .map(|app| app.client_id)
            .unwrap_or_default();
        let client_id = cx.new(|cx| {
            let mut input = InputState::new(window, cx).placeholder("Application ID");
            input.set_value(built_in, window, cx);
            input
        });
        for e in [&address, &token, &email, &client_id] {
            cx.observe(e, |_, _, cx| cx.notify()).detach();
        }
        let method = if Dispatcher::host_oauth_app(&default, None).is_some() {
            Method::Browser
        } else {
            Method::Token
        };
        let focus = match (kind, method) {
            (HostKind::Bitbucket, _) => email.read(cx).focus_handle(cx),
            (_, Method::Token) => token.read(cx).focus_handle(cx),
            (_, Method::Browser) => address.read(cx).focus_handle(cx),
        };
        window.focus(&focus, cx);
        Self {
            state,
            kind,
            method,
            address,
            token,
            email,
            client_id,
        }
    }

    /// The server the address box names, when it parses.
    fn endpoint(&self, cx: &App) -> Option<HostEndpoint> {
        let typed = self.address.read(cx).value().to_string();
        let typed = if typed.trim().is_empty() {
            self.kind.default_web_base().to_string()
        } else {
            typed
        };
        let allow_http = self
            .state
            .read(cx)
            .flags
            .bool(corvene_core::flags::ids::ENTERPRISE_PLAIN_HTTP);
        HostEndpoint::parse(self.kind, &typed, allow_http).ok()
    }

    /// Whether the browser can sign in here at all (Bitbucket only with an
    /// application baked into the build).
    fn browser_possible(&self, cx: &App) -> bool {
        match self.kind {
            HostKind::Bitbucket => self
                .endpoint(cx)
                .is_some_and(|e| Dispatcher::host_oauth_app(&e, None).is_some()),
            _ => true,
        }
    }

    fn set_method(&mut self, method: Method, window: &mut Window, cx: &mut Context<Self>) {
        Dispatcher::cancel_host_sign_in(cx);
        self.method = method;
        let focus = match method {
            Method::Token if self.kind == HostKind::Bitbucket => {
                self.email.read(cx).focus_handle(cx)
            }
            Method::Token => self.token.read(cx).focus_handle(cx),
            Method::Browser => self.client_id.read(cx).focus_handle(cx),
        };
        window.focus(&focus, cx);
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let address = self.address.read(cx).value().to_string();
        match self.method {
            Method::Token => {
                let token = self.token.read(cx).value().to_string();
                let email = (self.kind == HostKind::Bitbucket)
                    .then(|| self.email.read(cx).value().to_string());
                Dispatcher::host_sign_in_with_token(self.kind, address, token, email, cx);
            }
            Method::Browser => {
                let client_id = self.client_id.read(cx).value().to_string();
                Dispatcher::host_sign_in_with_browser(
                    self.kind,
                    address,
                    Some(client_id).filter(|id| !id.trim().is_empty()),
                    cx,
                );
            }
        }
    }

    fn link(
        id: &'static str,
        text: impl Into<SharedString>,
        on_click: impl Fn(&mut Window, &mut App) + 'static,
        cx: &App,
    ) -> Stateful<Div> {
        div()
            .id(id)
            .text_color(cx.ghd().link)
            .cursor_pointer()
            .child(text.into())
            .on_click(move |_, window, cx| on_click(window, cx))
    }
}

impl Render for SignInHostDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd();
        let close = |_: &mut Window, cx: &mut App| {
            Dispatcher::cancel_host_sign_in(cx);
            Dispatcher::close_popup(cx);
        };
        let this = cx.entity();
        let (loading, waiting, error) = self
            .state
            .read(cx)
            .hosts
            .sign_in
            .as_ref()
            .map_or((false, false, None), |s| {
                (s.loading, s.waiting_for_browser, s.error.clone())
            });
        let endpoint = self.endpoint(cx);
        let kind = self.kind;
        let host_name = endpoint
            .as_ref()
            .map_or_else(|| kind.name().to_string(), HostEndpoint::friendly_name);

        let address_field = (kind != HostKind::Bitbucket).then(|| {
            labeled(
                "Server address",
                text_box("sign-in-host-address", &self.address, None, window, cx),
                cx,
            )
        });

        let body: AnyElement = if waiting {
            div()
                .flex()
                .flex_col()
                .gap(SPACING())
                .child(format!(
                    "Authorise Corvene on the {host_name} page that opened in your browser. It \
                     sends you back here when you are done."
                ))
                .child(div().text_color(t.text_secondary).child(format!(
                    "Waiting for {host_name} to send the sign-in back to Corvene…"
                )))
                .into_any_element()
        } else {
            match self.method {
                Method::Token => {
                    let token_url = endpoint
                        .as_ref()
                        .map(corvene_hosts::urls::token_settings_url);
                    div()
                        .flex()
                        .flex_col()
                        .gap(SPACING())
                        .children(address_field)
                        .child(token_help(kind))
                        .when_some(token_url, |d, url| {
                            d.child(
                                div().flex().child(
                                    button("sign-in-host-create-token", "Create a token", cx)
                                        .child(
                                            octicon(Octicon::LinkExternal, t.secondary_button_text)
                                                .ml(SPACING_HALF()),
                                        )
                                        .on_click(move |_, _, cx| Dispatcher::open_url(&url, cx)),
                                ),
                            )
                        })
                        .when(kind == HostKind::Bitbucket, |d| {
                            d.child(labeled(
                                "Atlassian account email",
                                text_box("sign-in-host-email", &self.email, None, window, cx),
                                cx,
                            ))
                        })
                        .child(labeled(
                            match kind {
                                HostKind::Bitbucket => "API token",
                                HostKind::Gitea => "Access token",
                                _ => "Personal access token",
                            },
                            password_text_box("sign-in-host-token", &self.token, window, cx),
                            cx,
                        ))
                        .when_some(error.clone(), |d, m| {
                            d.child(div().text_color(t.error).child(m))
                        })
                        .when(self.browser_possible(cx), |d| {
                            let this = this.clone();
                            d.child(Self::link(
                                "sign-in-host-browser-link",
                                "Sign in with your browser instead",
                                move |window, cx| {
                                    this.update(cx, |d, cx| {
                                        d.set_method(Method::Browser, window, cx)
                                    })
                                },
                                cx,
                            ))
                        })
                        .into_any_element()
                }
                Method::Browser => {
                    let apps_url = endpoint
                        .as_ref()
                        .map(corvene_hosts::urls::oauth_app_settings_url);
                    let scopes = corvene_hosts::oauth::scopes(kind);
                    div()
                        .flex()
                        .flex_col()
                        .gap(SPACING())
                        .children(address_field)
                        .child(format!(
                            "Corvene will open {host_name} in your browser. Authorise Corvene \
                             there to sign in."
                        ))
                        .when(kind != HostKind::Bitbucket, |d| {
                            d.child(labeled(
                                "OAuth application ID",
                                text_box(
                                    "sign-in-host-client-id",
                                    &self.client_id,
                                    None,
                                    window,
                                    cx,
                                ),
                                cx,
                            ))
                            .child(
                                div().text_color(t.text_secondary).child(format!(
                                    "Register an application on the server with the redirect URI \
                                     http://127.0.0.1/{}{}, not confidential, then enter its ID.",
                                    if scopes.is_empty() {
                                        ""
                                    } else {
                                        " and the scopes "
                                    },
                                    scopes
                                )),
                            )
                        })
                        .when_some(
                            apps_url.filter(|_| kind != HostKind::Bitbucket),
                            |d, url| {
                                d.child(Self::link(
                                    "sign-in-host-apps-link",
                                    "Open the server's application settings",
                                    move |_, cx| Dispatcher::open_url(&url, cx),
                                    cx,
                                ))
                            },
                        )
                        .when_some(error.clone(), |d, m| {
                            d.child(div().text_color(t.error).child(m))
                        })
                        .child({
                            let this = this.clone();
                            Self::link(
                                "sign-in-host-token-link",
                                "Sign in with a token instead",
                                move |window, cx| {
                                    this.update(cx, |d, cx| d.set_method(Method::Token, window, cx))
                                },
                                cx,
                            )
                        })
                        .into_any_element()
                }
            }
        };

        let submit_label = match self.method {
            Method::Token => mac_or("Sign In", "Sign in"),
            Method::Browser => mac_or("Continue With Browser", "Continue with browser"),
        };
        let missing = match self.method {
            Method::Token => {
                self.token.read(cx).value().trim().is_empty()
                    || (kind == HostKind::Bitbucket
                        && self.email.read(cx).value().trim().is_empty()
                        && !self.token.read(cx).value().trim().starts_with("ATCTT"))
            }
            Method::Browser => {
                kind != HostKind::Bitbucket && self.client_id.read(cx).value().trim().is_empty()
            }
        };
        let mut buttons = vec![DialogButton {
            id: "sign-in-host-cancel",
            label: "Cancel".into(),
            primary: false,
            disabled: false,
            on_click: Box::new(close),
        }];
        if !waiting {
            buttons.push(DialogButton {
                id: "sign-in-host-submit",
                label: submit_label.into(),
                primary: true,
                disabled: loading || missing || endpoint.is_none(),
                on_click: Box::new(move |_, cx| this.update(cx, |d, cx| d.submit(cx))),
            });
        }
        dialog(
            "sign-in-host",
            title(kind),
            div().w_full().child(body),
            buttons,
            close,
            window,
            cx,
        )
    }
}
