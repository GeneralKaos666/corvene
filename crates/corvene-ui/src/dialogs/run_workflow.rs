//! Corvene `351-actions`: Run workflow, the Actions view's form for a
//! `workflow_dispatch` workflow (github.com's "Run workflow" drop-down; no
//! GitHub Desktop counterpart). "Use workflow from" picks a branch or tag;
//! the workflow file is read at that ref and its
//! `on.workflow_dispatch.inputs` become the fields (a text box for a string
//! or number, a checkbox for a boolean, a menu for a choice or an
//! environment), with their defaults. Run hands the values to
//! `Dispatcher::dispatch_actions_workflow`.

use std::collections::HashMap;

use corvene_core::github_actions::{DefinitionState, DispatchSpec, InputKind, actions_of};
use corvene_core::{AppState, Dispatcher, GitHubRepository, Popup};
use gpui_kit::component::input::InputState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::context_menu::mac_or;
use crate::dialog::{DialogButton, dialog};
use crate::icons::{Octicon, octicon, spin};
use crate::theme::ActiveGhdTheme;
use crate::theme::sizes::*;
use crate::widgets::{
    SelectHandler, SelectItem, checkbox_row, dialog_error_banner, labeled, select_button_items,
    text_box,
};

/// One input's value in the form.
enum Field {
    Text(Entity<InputState>),
    Bool(bool),
    /// A choice or an environment.
    Pick(String),
}

pub struct RunWorkflowDialog {
    state: Entity<AppState>,
    workflow: u64,
    git_ref: String,
    /// The definition the fields were built from.
    built_for: Option<(u64, String)>,
    fields: HashMap<String, Field>,
    /// Shown after a Run with invalid values.
    errors: Vec<String>,
}

impl RunWorkflowDialog {
    pub fn new(
        state: Entity<AppState>,
        github: GitHubRepository,
        workflow: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let git_ref = github
            .default_branch
            .clone()
            .unwrap_or_else(|| "main".into());
        crate::windows::observe_state_in(&state, window, cx, |this, _, window, cx| {
            this.sync_fields(window, cx);
            cx.notify();
        })
        .detach();
        let mut this = Self {
            state,
            workflow,
            git_ref,
            built_for: None,
            fields: HashMap::new(),
            errors: Vec::new(),
        };
        this.sync_fields(window, cx);
        this
    }

    fn spec(&self, cx: &App) -> Option<DispatchSpec> {
        let s = self.state.read(cx);
        match actions_of(s)?
            .definitions
            .get(&(self.workflow, self.git_ref.clone()))?
        {
            DefinitionState::Loaded(spec) => Some(spec.clone()),
            _ => None,
        }
    }

    /// Build the fields once the definition for the ref is read, keeping
    /// the values typed for inputs of the same name.
    fn sync_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = (self.workflow, self.git_ref.clone());
        if self.built_for.as_ref() == Some(&key) {
            return;
        }
        let Some(spec) = self.spec(cx) else { return };
        self.built_for = Some(key);
        let mut old = std::mem::take(&mut self.fields);
        let mut first_text: Option<Entity<InputState>> = None;
        for input in &spec.inputs {
            let kept = old.remove(&input.name);
            let field = match (&input.kind, kept) {
                (InputKind::String | InputKind::Number, Some(Field::Text(state))) => {
                    Field::Text(state)
                }
                (InputKind::String | InputKind::Number, _) => {
                    let initial = input.initial_value();
                    let placeholder = input.default.clone().unwrap_or_default();
                    let state = cx.new(|cx| {
                        InputState::new(window, cx)
                            .placeholder(placeholder)
                            .default_value(initial)
                    });
                    cx.observe(&state, |_, _, cx| cx.notify()).detach();
                    Field::Text(state)
                }
                (InputKind::Boolean, Some(Field::Bool(b))) => Field::Bool(b),
                (InputKind::Boolean, _) => Field::Bool(input.initial_value() == "true"),
                (_, Some(Field::Pick(p))) => Field::Pick(p),
                (_, _) => Field::Pick(input.initial_value()),
            };
            if first_text.is_none()
                && let Field::Text(state) = &field
            {
                first_text = Some(state.clone());
            }
            self.fields.insert(input.name.clone(), field);
        }
        if let Some(state) = first_text {
            let handle = state.read(cx).focus_handle(cx);
            window.focus(&handle, cx);
        }
    }

    fn value(&self, name: &str, cx: &App) -> String {
        match self.fields.get(name) {
            Some(Field::Text(state)) => state.read(cx).value().trim().to_string(),
            Some(Field::Bool(b)) => b.to_string(),
            Some(Field::Pick(p)) => p.clone(),
            None => String::new(),
        }
    }

    fn set_ref(&mut self, git_ref: String, cx: &mut Context<Self>) {
        if self.git_ref == git_ref {
            return;
        }
        self.git_ref = git_ref.clone();
        self.errors.clear();
        Dispatcher::load_workflow_definition(self.workflow, git_ref, cx);
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let Some(spec) = self.spec(cx) else { return };
        let mut values = Vec::new();
        self.errors.clear();
        for input in &spec.inputs {
            let value = self.value(&input.name, cx);
            if let Some(err) = input.validate(&value) {
                self.errors.push(err);
            }
            // an empty optional value takes the file's default
            if !value.is_empty() || input.kind == InputKind::Boolean {
                values.push((input.name.clone(), value));
            }
        }
        if !self.errors.is_empty() {
            cx.notify();
            return;
        }
        Dispatcher::dispatch_actions_workflow(self.workflow, self.git_ref.clone(), values, cx);
    }

    fn close(_: &mut Window, cx: &mut App) {
        Dispatcher::close_popup_if(|p| matches!(p, Popup::RunWorkflow { .. }), cx);
    }
}

impl Render for RunWorkflowDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = cx.ghd().clone();
        let this = cx.weak_entity();
        let (view_refs, definition, workflow, environments, dispatching, dispatch_error, blocker) = {
            let s = self.state.read(cx);
            match actions_of(s) {
                Some(v) => (
                    v.refs.clone(),
                    v.definitions
                        .get(&(self.workflow, self.git_ref.clone()))
                        .cloned(),
                    v.workflow(self.workflow).cloned(),
                    v.environments.clone(),
                    v.dispatching,
                    v.dispatch_error.clone(),
                    v.write_blocker(),
                ),
                None => (None, None, None, None, false, None, None),
            }
        };
        let workflow_name = workflow
            .as_ref()
            .map(|w| w.name.clone())
            .unwrap_or_default();
        // Use workflow from
        let mut refs = view_refs.clone().unwrap_or_default();
        if !refs.iter().any(|r| r.name == self.git_ref) {
            refs.insert(
                0,
                corvene_core::github_actions::RefChoice {
                    name: self.git_ref.clone(),
                    tag: false,
                },
            );
        }
        let mut items: Vec<SelectItem> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        let branches: Vec<&str> = refs
            .iter()
            .filter(|r| !r.tag)
            .map(|r| r.name.as_str())
            .collect();
        let tags: Vec<&str> = refs
            .iter()
            .filter(|r| r.tag)
            .map(|r| r.name.as_str())
            .collect();
        for b in &branches {
            items.push(SelectItem::Option(SharedString::from(b.to_string())));
            names.push(b.to_string());
        }
        if !tags.is_empty() {
            items.push(SelectItem::Separator);
            for tag in &tags {
                items.push(SelectItem::Option(SharedString::from(format!(
                    "{tag} (tag)"
                ))));
                names.push(tag.to_string());
            }
        }
        let selected = names.iter().position(|n| *n == self.git_ref);
        let on_ref: SelectHandler = std::rc::Rc::new({
            let this = this.clone();
            let names = names.clone();
            move |ix, _, cx| {
                if let Some(name) = names.get(ix).cloned() {
                    this.update(cx, |d, cx| d.set_ref(name, cx)).ok();
                }
            }
        });
        let ref_picker = labeled(
            "Use workflow from",
            select_button_items(
                "run-workflow-ref",
                self.git_ref.clone(),
                items,
                selected,
                dispatching,
                on_ref,
                cx,
            ),
            cx,
        );
        let mut runnable = false;
        let status: Option<AnyElement> = match &definition {
            None | Some(DefinitionState::Loading) => Some(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .text_color(t.text_secondary)
                    .child(spin(
                        octicon(Octicon::SyncClockwise, t.text_secondary),
                        "run-workflow-spin",
                    ))
                    .child(format!("Reading the workflow file on {}…", self.git_ref))
                    .into_any_element(),
            ),
            Some(DefinitionState::Missing) => Some(
                div()
                    .text_color(t.dialog_warning)
                    .child(format!(
                        "{} is not on {}, so it cannot run there.",
                        workflow
                            .as_ref()
                            .map(|w| w.path.clone())
                            .unwrap_or_else(|| "The workflow file".into()),
                        self.git_ref
                    ))
                    .into_any_element(),
            ),
            Some(DefinitionState::Failed(message)) => Some(
                div()
                    .text_color(t.dialog_error)
                    .child(message.clone())
                    .into_any_element(),
            ),
            Some(DefinitionState::Loaded(spec)) if !spec.dispatchable => Some(
                div()
                    .text_color(t.dialog_warning)
                    .child(format!(
                        "On {}, {workflow_name} has no workflow_dispatch trigger, so it cannot be run by hand.",
                        self.git_ref
                    ))
                    .into_any_element(),
            ),
            Some(DefinitionState::Loaded(_)) => {
                runnable = true;
                None
            }
        };
        let spec = match &definition {
            Some(DefinitionState::Loaded(spec)) if spec.dispatchable => Some(spec.clone()),
            _ => None,
        };
        let mut fields: Vec<AnyElement> = Vec::new();
        for (ix, input) in spec.iter().flat_map(|s| s.inputs.iter()).enumerate() {
            let label = if input.required {
                format!("{} *", input.label())
            } else {
                input.label().to_string()
            };
            let name = input.name.clone();
            let element: AnyElement = match (self.fields.get(&input.name), &input.kind) {
                (Some(Field::Text(state)), _) => labeled(
                    label,
                    text_box(("run-workflow-input", ix), state, None, window, cx),
                    cx,
                )
                .into_any_element(),
                (Some(Field::Bool(on)), _) => {
                    let this = this.clone();
                    div()
                        .id(("run-workflow-bool", ix))
                        .child(checkbox_row(
                            "run-workflow-bool",
                            *on,
                            label,
                            move |on, _, cx| {
                                let name = name.clone();
                                this.update(cx, |d, cx| {
                                    d.fields.insert(name, Field::Bool(on));
                                    cx.notify();
                                })
                                .ok();
                            },
                            cx,
                        ))
                        .into_any_element()
                }
                (Some(Field::Pick(current)), kind) => {
                    let mut options: Vec<String> = match kind {
                        InputKind::Choice(options) => options.clone(),
                        _ => environments.clone().unwrap_or_default(),
                    };
                    if !input.required {
                        options.insert(0, String::new());
                    }
                    if !current.is_empty() && !options.contains(current) {
                        options.push(current.clone());
                    }
                    let items: Vec<SelectItem> = options
                        .iter()
                        .map(|o| {
                            SelectItem::Option(if o.is_empty() {
                                "(none)".into()
                            } else {
                                SharedString::from(o.clone())
                            })
                        })
                        .collect();
                    let selected = options.iter().position(|o| o == current);
                    let this = this.clone();
                    let picked = options.clone();
                    let on_pick: SelectHandler = std::rc::Rc::new(move |ix, _, cx| {
                        let Some(value) = picked.get(ix).cloned() else {
                            return;
                        };
                        let name = name.clone();
                        this.update(cx, |d, cx| {
                            d.fields.insert(name, Field::Pick(value));
                            cx.notify();
                        })
                        .ok();
                    });
                    let shown = if current.is_empty() {
                        "(none)".to_string()
                    } else {
                        current.clone()
                    };
                    labeled(
                        label,
                        select_button_items(
                            ("run-workflow-pick", ix),
                            shown,
                            items,
                            selected,
                            false,
                            on_pick,
                            cx,
                        ),
                        cx,
                    )
                    .into_any_element()
                }
                (None, _) => div().into_any_element(),
            };
            fields.push(element);
        }
        let errors = self.errors.clone();
        let content = div()
            .w(crate::theme::fit_width(460.))
            .flex()
            .flex_col()
            .gap(SPACING())
            .when_some(dispatch_error, |d, message| {
                d.child(dialog_error_banner(message, cx))
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(SPACING_HALF())
                    .text_color(t.text_secondary)
                    .child(octicon(Octicon::Workflow, t.text_secondary))
                    .child(
                        div().truncate().child(
                            workflow
                                .as_ref()
                                .map(|w| format!("{} · {}", w.name, w.path))
                                .unwrap_or_default(),
                        ),
                    ),
            )
            .child(ref_picker)
            .children(status)
            .when(spec.as_ref().is_some_and(|s| s.inputs.is_empty()), |d| {
                d.child(
                    div()
                        .text_color(t.text_secondary)
                        .child("This workflow takes no inputs."),
                )
            })
            .children(fields)
            .children(errors.into_iter().map(|e| {
                div()
                    .text_size(FONT_SIZE_SM())
                    .text_color(t.dialog_error)
                    .child(e)
            }))
            .when_some(blocker.clone(), |d, why| {
                d.child(
                    div()
                        .text_size(FONT_SIZE_SM())
                        .text_color(t.dialog_warning)
                        .child(why),
                )
            });
        let disabled = !runnable || dispatching || blocker.is_some();
        dialog(
            "dialog-run-workflow",
            mac_or("Run Workflow", "Run workflow"),
            content,
            vec![
                DialogButton {
                    id: "run-workflow-cancel",
                    label: "Cancel".into(),
                    primary: false,
                    disabled: false,
                    on_click: Box::new(Self::close),
                },
                DialogButton {
                    id: "run-workflow-run",
                    label: if dispatching {
                        "Starting…".into()
                    } else {
                        mac_or("Run Workflow", "Run workflow").into()
                    },
                    primary: true,
                    disabled,
                    on_click: Box::new(move |_, cx| {
                        if !disabled {
                            this.update(cx, |d, cx| d.submit(cx)).ok();
                        }
                    }),
                },
            ],
            Self::close,
            window,
            cx,
        )
    }
}
