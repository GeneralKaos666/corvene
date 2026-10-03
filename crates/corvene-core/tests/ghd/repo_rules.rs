//! Port of GitHub Desktop's `app/test/unit/repo-rules-test.ts`.
//!
//! - GitHub Desktop's `parseRepoRules(rules, rulesets, repository)`
//!   (`lib/helpers/repo-rules.ts`) is `corvene_core::repo_rules::parse_repo_rules`.
//!   GitHub Desktop passes the rulesets (`IAPIRepoRuleset`) and decides
//!   `enforced` (`current_user_can_bypass === 'always' ? 'bypass' : true`)
//!   inside; Corvene's caller (`Dispatcher::refresh_branch_protection`)
//!   keeps `ApiRepoRuleset::enforced()` per ruleset id and passes that map,
//!   so [`parse_repo_rules_for`] does the same. GitHub Desktop reads
//!   `commit.gpgsign` from the repository only for a `required_signatures`
//!   rule; Corvene's caller reads it first and passes it. No case has such a
//!   rule (GitHub Desktop's `new Repository('repo1', 1, null, false)` is
//!   never read), so `false` stands for it.
//! - `IAPIRepoRule` / `IAPIRepoRuleset` are `corvene_github::ApiRepoRule` /
//!   `ApiRepoRuleset`, `APIRepoRuleType.X` is the rule's `kind` string and
//!   `APIRepoRuleMetadataOperator` is `corvene_models::RuleOperator`.
//! - `RepoRulesInfo.commitMessagePatterns` (`RepoRulesMetadataRules`) is a
//!   `Vec<RepoRulesMetadataRule>`: `hasRules` is `!is_empty()` and
//!   `getFailedRules(text)` is `corvene_core::failed_rules(&rules, text)`
//!   (imported as `get_failed_rules`).
//!   A `RepoRuleEnforced` is truthy when it is not `No` (`true` or
//!   `'bypass'`).
//! - `getEnforcedRuleDescriptions` (`lib/stores/copilot-store.ts`) only
//!   feeds the Copilot commit message prompt; Copilot is left out by design,
//!   so those cases are skipped (`tools/ghd-tests/skips/api.tsv`).

use std::collections::HashMap;

use RepoRulesMetadataStatus::{Bypass, Fail, Pass};
use corvene_core::failed_rules as get_failed_rules;
use corvene_core::repo_rules::parse_repo_rules;
use corvene_github::api::ApiRepoRuleParameters;
use corvene_github::{ApiRepoRule, ApiRepoRuleset};
use corvene_models::{
    RepoRuleEnforced, RepoRulesInfo, RepoRulesMetadataFailures, RepoRulesMetadataStatus,
    RuleOperator,
};

fn creation_rule() -> ApiRepoRule {
    ApiRepoRule {
        ruleset_id: 1,
        kind: "creation".into(),
        parameters: None,
    }
}

fn creation_bypass_always_rule() -> ApiRepoRule {
    ApiRepoRule {
        ruleset_id: 2,
        kind: "creation".into(),
        parameters: None,
    }
}

fn creation_bypass_pull_requests_only_rule() -> ApiRepoRule {
    ApiRepoRule {
        ruleset_id: 3,
        kind: "creation".into(),
        parameters: None,
    }
}

fn commit_message_pattern(
    ruleset_id: u64,
    negate: bool,
    pattern: &str,
    operator: RuleOperator,
) -> ApiRepoRule {
    ApiRepoRule {
        ruleset_id,
        kind: "commit_message_pattern".into(),
        parameters: Some(ApiRepoRuleParameters {
            name: String::new(),
            negate,
            pattern: pattern.into(),
            operator,
        }),
    }
}

fn commit_message_pattern_starts_with_rule() -> ApiRepoRule {
    commit_message_pattern(1, false, "abc", RuleOperator::StartsWith)
}

fn commit_message_pattern_starts_with_bypass_rule() -> ApiRepoRule {
    commit_message_pattern(2, false, "abc", RuleOperator::StartsWith)
}

fn commit_message_pattern_special_characters_rule() -> ApiRepoRule {
    // API response is backslash escaped like this
    commit_message_pattern(1, false, r"(a.b.c.)|(d+)\d", RuleOperator::StartsWith)
}

fn commit_message_pattern_ends_with_rule() -> ApiRepoRule {
    commit_message_pattern(1, true, "end", RuleOperator::EndsWith)
}

fn commit_message_pattern_contains_rule() -> ApiRepoRule {
    commit_message_pattern(1, true, "con", RuleOperator::Contains)
}

fn commit_message_pattern_regex_rule1() -> ApiRepoRule {
    commit_message_pattern(1, false, "(a.b.c.)|(d+)", RuleOperator::RegexMatch)
}

fn commit_message_pattern_regex_rule2() -> ApiRepoRule {
    // API response is backslash escaped like this
    commit_message_pattern(1, false, r"^\A(d|e)oo\d$", RuleOperator::RegexMatch)
}

fn commit_message_pattern_regex_multi_line_rule() -> ApiRepoRule {
    commit_message_pattern(1, false, "(?m)^foo", RuleOperator::RegexMatch)
}

fn rulesets() -> HashMap<u64, ApiRepoRuleset> {
    HashMap::from([
        (
            1,
            ApiRepoRuleset {
                id: 1,
                current_user_can_bypass: Some("never".into()),
            },
        ),
        (
            2,
            ApiRepoRuleset {
                id: 2,
                current_user_can_bypass: Some("always".into()),
            },
        ),
    ])
}

/// GitHub Desktop's `await parseRepoRules(rules, rulesets, repo)`.
fn parse_repo_rules_for(
    rules: &[ApiRepoRule],
    rulesets: &HashMap<u64, ApiRepoRuleset>,
) -> RepoRulesInfo {
    let enforced: HashMap<u64, RepoRuleEnforced> = rulesets
        .iter()
        .map(|(id, ruleset)| (*id, ruleset.enforced()))
        .collect();
    parse_repo_rules(rules, &enforced, false)
}

/// JavaScript truthiness of a `RepoRuleEnforced` (`true` or `'bypass'`).
fn truthy(enforced: RepoRuleEnforced) -> bool {
    enforced != RepoRuleEnforced::No
}

fn validate_metadata_rules(
    rules: &RepoRulesMetadataFailures,
    status: RepoRulesMetadataStatus,
    bypasses_expected: usize,
    failures_expected: usize,
) {
    assert_eq!(rules.status(), status);
    assert_eq!(rules.bypassed.len(), bypasses_expected);
    assert_eq!(rules.failed.len(), failures_expected);
}

mod parse_repo_rules {
    use super::*;

    // GHD: unit/repo-rules-test.ts › await parseRepoRules › cannot bypass when bypass is "never"
    #[test]
    fn cannot_bypass_when_bypass_is_never() {
        // the creation rule references ruleset ID 1, which has a bypass of 'never'
        let rules = [creation_rule()];
        let result = parse_repo_rules_for(&rules, &rulesets());
        assert!(truthy(result.creation_restricted));
    }

    // GHD: unit/repo-rules-test.ts › await parseRepoRules › can bypass when bypass is "always"
    #[test]
    fn can_bypass_when_bypass_is_always() {
        // the creationBypass rule references ruleset ID 2, which has a bypass of 'always'
        let rules = [creation_bypass_always_rule()];
        let result = parse_repo_rules_for(&rules, &rulesets());
        assert_eq!(result.creation_restricted, RepoRuleEnforced::Bypass);
    }

    // GHD: unit/repo-rules-test.ts › await parseRepoRules › cannot bypass when at least one bypass mode is "never" or "pull_requests_only"
    #[test]
    fn cannot_bypass_when_at_least_one_bypass_mode_is_never_or_pull_requests_only() {
        let rules = [creation_rule(), creation_bypass_always_rule()];
        let result = parse_repo_rules_for(&rules, &rulesets());
        assert!(truthy(result.creation_restricted));

        let rules2 = [creation_rule(), creation_bypass_pull_requests_only_rule()];
        let result2 = parse_repo_rules_for(&rules2, &rulesets());
        assert!(truthy(result2.creation_restricted));
    }

    // GHD: unit/repo-rules-test.ts › await parseRepoRules › is not enforced when no rules are provided
    #[test]
    fn is_not_enforced_when_no_rules_are_provided() {
        let rules: [ApiRepoRule; 0] = [];
        let repo_rules_info = parse_repo_rules_for(&rules, &rulesets());
        assert!(!truthy(repo_rules_info.creation_restricted));
    }
}

mod repo_metadata_rules {
    use super::*;

    mod starts_with_rule {
        use super::*;

        // GHD: unit/repo-rules-test.ts › repo metadata rules › startsWith rule › shows no rules and passes everything when no rules are provided
        #[test]
        fn shows_no_rules_and_passes_everything_when_no_rules_are_provided() {
            let rules: [ApiRepoRule; 0] = [];
            let repo_rules_info = parse_repo_rules_for(&rules, &rulesets());
            assert!(repo_rules_info.commit_message_patterns.is_empty());

            let failed_rules = get_failed_rules(&repo_rules_info.commit_message_patterns, "abc");
            validate_metadata_rules(&failed_rules, Pass, 0, 0);
        }

        // GHD: unit/repo-rules-test.ts › repo metadata rules › startsWith rule › has correct matching logic for StartsWith rule
        #[test]
        fn has_correct_matching_logic_for_starts_with_rule() {
            let rules = [commit_message_pattern_starts_with_rule()];
            let repo_rules_info = parse_repo_rules_for(&rules, &rulesets());
            assert!(!repo_rules_info.commit_message_patterns.is_empty());

            let failed_rules = get_failed_rules(&repo_rules_info.commit_message_patterns, "def");
            validate_metadata_rules(&failed_rules, Fail, 0, 1);
            assert_eq!(
                failed_rules.failed[0].description,
                "must start with \"abc\""
            );
        }

        // GHD: unit/repo-rules-test.ts › repo metadata rules › startsWith rule › has correct bypass logic for StartsWith rule
        #[test]
        fn has_correct_bypass_logic_for_starts_with_rule() {
            let rules = [commit_message_pattern_starts_with_bypass_rule()];
            let repo_rules_info = parse_repo_rules_for(&rules, &rulesets());

            let failed_rules = get_failed_rules(&repo_rules_info.commit_message_patterns, "def");
            validate_metadata_rules(&failed_rules, Bypass, 1, 0);
            assert_eq!(
                failed_rules.bypassed[0].description,
                "must start with \"abc\""
            );
        }

        // GHD: unit/repo-rules-test.ts › repo metadata rules › startsWith rule › has correct logic when bypassed rule is included with non-bypassed rule
        #[test]
        fn has_correct_logic_when_bypassed_rule_is_included_with_non_bypassed_rule() {
            let rules = [
                commit_message_pattern_starts_with_rule(),
                commit_message_pattern_starts_with_bypass_rule(),
            ];
            let repo_rules_info = parse_repo_rules_for(&rules, &rulesets());

            let failed_rules = get_failed_rules(&repo_rules_info.commit_message_patterns, "def");
            validate_metadata_rules(&failed_rules, Fail, 1, 1);
            assert_eq!(
                failed_rules.bypassed[0].description,
                "must start with \"abc\""
            );
            assert_eq!(
                failed_rules.failed[0].description,
                "must start with \"abc\""
            );
        }

        // GHD: unit/repo-rules-test.ts › repo metadata rules › startsWith rule › escapes special characters and otherwise handles regex properly
        #[test]
        fn escapes_special_characters_and_otherwise_handles_regex_properly() {
            let rules = [commit_message_pattern_special_characters_rule()];
            let repo_rules_info = parse_repo_rules_for(&rules, &rulesets());

            // if the . in the pattern is interpreted as a regex special character, this will pass
            let rules1 = get_failed_rules(&repo_rules_info.commit_message_patterns, "aabbcc");
            assert_eq!(rules1.status(), Fail);

            let rules2 = get_failed_rules(&repo_rules_info.commit_message_patterns, "dd");
            assert_eq!(rules2.status(), Fail);

            let passed_rules =
                get_failed_rules(&repo_rules_info.commit_message_patterns, r"(a.b.c.)|(d+)\d");
            assert_eq!(passed_rules.status(), Pass);
        }
    }

    mod ends_with_rule {
        use super::*;

        // GHD: unit/repo-rules-test.ts › repo metadata rules › endsWith rule › has correct matching logic for negated EndsWith rule
        #[test]
        fn has_correct_matching_logic_for_negated_ends_with_rule() {
            let rules = [commit_message_pattern_ends_with_rule()];
            let repo_rules_info = parse_repo_rules_for(&rules, &rulesets());

            let failed_rules = get_failed_rules(&repo_rules_info.commit_message_patterns, "end");
            validate_metadata_rules(&failed_rules, Fail, 0, 1);
            assert_eq!(
                failed_rules.failed[0].description,
                "must not end with \"end\""
            );

            let passed_rules = get_failed_rules(&repo_rules_info.commit_message_patterns, "abc");
            validate_metadata_rules(&passed_rules, Pass, 0, 0);
        }
    }

    mod contains_rule {
        use super::*;

        // GHD: unit/repo-rules-test.ts › repo metadata rules › contains rule › has correct matching logic for Contains rule
        #[test]
        fn has_correct_matching_logic_for_contains_rule() {
            let rules = [commit_message_pattern_contains_rule()];
            let repo_rules_info = parse_repo_rules_for(&rules, &rulesets());

            let failed_rules =
                get_failed_rules(&repo_rules_info.commit_message_patterns, "fooconbar");
            validate_metadata_rules(&failed_rules, Fail, 0, 1);
            assert_eq!(
                failed_rules.failed[0].description,
                "must not contain \"con\""
            );

            let passed_rules = get_failed_rules(&repo_rules_info.commit_message_patterns, "foobar");
            validate_metadata_rules(&passed_rules, Pass, 0, 0);
        }
    }

    mod regex_rule {
        use super::*;

        // GHD: unit/repo-rules-test.ts › repo metadata rules › regex rule › has correct matching logic for RegexMatch rule
        #[test]
        fn has_correct_matching_logic_for_regex_match_rule() {
            let rules = [
                commit_message_pattern_regex_rule1(),
                commit_message_pattern_regex_rule2(),
            ];
            let repo_rules_info = parse_repo_rules_for(&rules, &rulesets());
            let patterns = &repo_rules_info.commit_message_patterns;

            let results1 = get_failed_rules(patterns, "doo5");
            validate_metadata_rules(&results1, Pass, 0, 0);

            let results2 = get_failed_rules(patterns, "afbgch");
            validate_metadata_rules(&results2, Fail, 0, 1);
            assert_eq!(
                results2.failed[0].description,
                r#"must match the regular expression "^\A(d|e)oo\d$""#
            );

            let results3 = get_failed_rules(patterns, "eoo4");
            validate_metadata_rules(&results3, Fail, 0, 1);
            assert_eq!(
                results3.failed[0].description,
                r#"must match the regular expression "(a.b.c.)|(d+)""#
            );

            let results4 = get_failed_rules(patterns, "fgsa");
            validate_metadata_rules(&results4, Fail, 0, 2);
            assert_eq!(
                results4.failed[0].description,
                r#"must match the regular expression "(a.b.c.)|(d+)""#
            );
            assert_eq!(
                results4.failed[1].description,
                r#"must match the regular expression "^\A(d|e)oo\d$""#
            );
        }

        // GHD: unit/repo-rules-test.ts › repo metadata rules › regex rule › has correct matching logic for multi-line data
        #[test]
        fn has_correct_matching_logic_for_multi_line_data() {
            let rules = [commit_message_pattern_regex_multi_line_rule()];
            let repo_rules_info = parse_repo_rules_for(&rules, &rulesets());
            let patterns = &repo_rules_info.commit_message_patterns;

            let results1 = get_failed_rules(patterns, "first line\nfoo");
            validate_metadata_rules(&results1, Pass, 0, 0);

            let results2 = get_failed_rules(patterns, "asdf\nbar");
            validate_metadata_rules(&results2, Fail, 0, 1);
            assert_eq!(
                results2.failed[0].description,
                r#"must match the regular expression "(?m)^foo""#
            );
        }
    }
}
