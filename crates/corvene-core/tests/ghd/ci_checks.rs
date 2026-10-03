//! Port of GitHub Desktop's `app/test/unit/ci-checks-test.ts`.
//!
//! GitHub Desktop's `getCheckRunsGroupedByActionWorkflowNameAndEvent(checkRuns)`
//! (`lib/ci-checks/ci-checks.ts`) is `corvene_core::group_check_runs`,
//! which also sorts the groups as `getCheckRunGroupNames` does; the cases
//! only look at which group names exist (`[...groups.keys()]`). GitHub
//! Desktop's `IRefCheck` is `corvene_models::RefCheck` and its
//! `actionsWorkflow` (`IAPIWorkflowRun`) is `corvene_models::WorkflowRun`.

use corvene_core::group_check_runs;
use corvene_models::{CheckConclusion, CheckStatus, RefCheck, WorkflowRun};

/// The group names, `[...groups.keys()]`.
fn group_names_of(check_runs: &[RefCheck]) -> Vec<String> {
    group_check_runs(check_runs)
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

fn includes(names: &[String], name: &str) -> bool {
    names.iter().any(|n| n == name)
}

// GHD: unit/ci-checks-test.ts › getCheckRunsGroupedByActionWorkflowNameAndEvent › groups by actions workflow name
#[test]
fn groups_by_actions_workflow_name() {
    let check_runs = [
        build_mock_check_run("1", "", Some("test1"), None),
        build_mock_check_run("1", "", Some("test2"), None),
    ];
    let group_names = group_names_of(&check_runs);
    assert!(includes(&group_names, "test1"));
    assert!(includes(&group_names, "test2"));
}

// GHD: unit/ci-checks-test.ts › getCheckRunsGroupedByActionWorkflowNameAndEvent › groups any check run without an actions workflow name into Other
#[test]
fn groups_any_check_run_without_an_actions_workflow_name_into_other() {
    let check_runs = [
        build_mock_check_run("1", "", Some("test1"), None),
        build_mock_check_run("1", "", None, None),
    ];
    let group_names = group_names_of(&check_runs);
    assert!(includes(&group_names, "test1"));
    assert!(includes(&group_names, "Other"));
}

// GHD: unit/ci-checks-test.ts › getCheckRunsGroupedByActionWorkflowNameAndEvent › groups any check run without an actions workflow name with an app name of "GitHub Code Scanning" into "Code scanning results"
#[test]
fn groups_code_scanning_check_runs_without_a_workflow_into_code_scanning_results() {
    let check_runs = [
        build_mock_check_run("1", "", Some("test1"), None),
        build_mock_check_run("1", "", None, None),
        build_mock_check_run("1", "GitHub Code Scanning", None, None),
    ];
    let group_names = group_names_of(&check_runs);
    assert!(includes(&group_names, "test1"));
    assert!(includes(&group_names, "Other"));
    assert!(includes(&group_names, "Code scanning results"));
}

// GHD: unit/ci-checks-test.ts › getCheckRunsGroupedByActionWorkflowNameAndEvent › groups by actions event type if more than one event type
#[test]
fn groups_by_actions_event_type_if_more_than_one_event_type() {
    let mut check_runs = vec![
        build_mock_check_run("1", "", Some("test1"), None),
        build_mock_check_run("1", "", Some("test2"), None),
    ];
    let mut group_names = group_names_of(&check_runs);

    // no event types
    assert!(includes(&group_names, "test1"));
    assert!(includes(&group_names, "test2"));

    check_runs.push(build_mock_check_run(
        "1",
        "",
        Some("test3"),
        Some("pull_request"),
    ));
    group_names = group_names_of(&check_runs);

    // only one event
    assert!(includes(&group_names, "test1"));
    assert!(includes(&group_names, "test2"));
    assert!(includes(&group_names, "test3"));

    check_runs.push(build_mock_check_run("1", "", Some("test4"), Some("push")));
    group_names = group_names_of(&check_runs);

    // two event types for test3 and test4
    assert!(includes(&group_names, "test1"));
    assert!(includes(&group_names, "test2"));
    assert!(includes(&group_names, "test3 (pull_request)"));
    assert!(includes(&group_names, "test4 (push)"));
}

fn build_mock_check_run(
    name: &str,
    app_name: &str,
    action_workflow_name: Option<&str>,
    action_workflow_event: Option<&str>,
) -> RefCheck {
    // `actionWorkflowName || actionWorkflowEvent`: empty strings count as unset
    let name_set = action_workflow_name.is_some_and(|n| !n.is_empty());
    let event_set = action_workflow_event.is_some_and(|e| !e.is_empty());
    RefCheck {
        id: 1,
        name: name.to_string(),
        description: String::new(),
        status: CheckStatus::Completed,
        conclusion: Some(CheckConclusion::Success),
        app_name: app_name.to_string(),
        html_url: None,
        check_suite_id: None,
        actions_workflow: (name_set || event_set).then(|| WorkflowRun {
            name: action_workflow_name.unwrap_or_default().to_string(),
            event: action_workflow_event.unwrap_or_default().to_string(),
            id: 1,
            workflow_id: 1,
            created_at: String::new(),
            check_suite_id: Some(1),
        }),
        job_steps: None,
    }
}
