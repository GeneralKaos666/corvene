//! Port of GitHub Desktop's `app/test/unit/main-process/menu-test.ts`
//! (`main-process/menu/build-default-menu.ts`, `ensure-item-ids.ts`).
//!
//! - `getAccessKey(label)` is a helper of the test file itself (the
//!   Windows-style access key after an unescaped `&`, lower-cased).
//!   Corvene's counterpart off macOS is the parser its in-window menu bar
//!   uses for the same `&` labels: `corvene_ui::views_menu::parse_mnemonic`
//!   (`&&` is a literal ampersand) and `views_menu::mnemonic_char` (the key,
//!   lower-cased). There [`get_access_key`] calls them, so the
//!   `getAccessKey` cases check Corvene's parser against GitHub Desktop's
//!   expectations. On macOS Corvene's labels carry no access keys (like
//!   GitHub Desktop's `__DARWIN__` labels) and `views_menu` is not built, so
//!   [`get_access_key`] is GitHub Desktop's regular expression ported as it
//!   is. `findDuplicateAccessKeys` (also the test file's) is ported as it
//!   is, on top of [`get_access_key`].
//! - `buildDefaultMenuTemplate(labels)` is Corvene's `menus::install(cx,
//!   options)` (`crates/corvene/src/menus.rs`, the same items with `&`
//!   mnemonics off macOS). It lives in the binary crate, which integration
//!   tests cannot reach, and builds GPUI menus inside an `App`;
//!   [`build_default_menu_template`] is a stand-in returning the template as
//!   GitHub Desktop's `MenuItemConstructorOptions` ([`MenuItem`]), and
//!   [`MenuLabelsEvent`] stands for GitHub Desktop's `MenuLabelsEvent`
//!   (Corvene's `menus::MenuOptions` holds the editor and shell labels and
//!   flags instead).
//! - `ensureItemIds` gives Electron menu items without an id one made from
//!   their labels; Corvene's menu items are GPUI actions and the fixed
//!   `corvene_core::menu_state::MenuId`s, so those cases are skipped
//!   (`tools/ghd-tests/skips/platform.tsv`).

/// GitHub Desktop's `Electron.MenuItemConstructorOptions`, the fields the
/// test reads.
#[derive(Clone, Debug, Default)]
#[allow(dead_code)] // built by the real buildDefaultMenuTemplate
struct MenuItem {
    label: Option<String>,
    /// `type: 'separator'`
    separator: bool,
    /// `visible` (`undefined` is visible)
    visible: Option<bool>,
    submenu: Option<Vec<MenuItem>>,
}

/// GitHub Desktop's `MenuLabelsEvent` (`models/menu-labels.ts`).
#[derive(Clone, Debug, Default)]
#[allow(dead_code)] // read by the real buildDefaultMenuTemplate
struct MenuLabelsEvent {
    selected_shell: Option<String>,
    selected_external_editor: Option<String>,
    ask_for_confirmation_on_force_push: bool,
    ask_for_confirmation_on_repository_removal: bool,
    is_stashed_changes_visible: bool,
    is_changes_filter_visible: bool,
    has_current_pull_request: bool,
    ask_for_confirmation_when_stashing_all_changes: bool,
    is_force_push_for_current_repository: bool,
}

/// Stand-in for GitHub Desktop's `buildDefaultMenuTemplate(labels)`
/// (`main-process/menu/build-default-menu.ts`). Replace it with the Corvene
/// function once one is reachable and remove the `#[ignore]`.
fn build_default_menu_template(_params: &MenuLabelsEvent) -> Vec<MenuItem> {
    unimplemented!(
        "buildDefaultMenuTemplate's counterpart (menus::install) is in the binary crate corvene"
    )
}

/// Extract the Windows-style access key from a menu item label, if any
/// (GitHub Desktop's `/(?<!&)&([^&])/`, lower-cased), through Corvene's
/// menu bar parser.
#[cfg(not(target_os = "macos"))]
fn get_access_key(label: &str) -> Option<String> {
    let (text, mnemonic) = corvene_ui::views_menu::parse_mnemonic(label);
    corvene_ui::views_menu::mnemonic_char(&text, mnemonic).map(String::from)
}

/// Extract the Windows-style access key from a menu item label, if any:
/// GitHub Desktop's `/(?<!&)&([^&])/`, lower-cased, as it is (macOS has no
/// Corvene parser to call, see the module doc).
#[cfg(target_os = "macos")]
fn get_access_key(label: &str) -> Option<String> {
    let chars: Vec<char> = label.chars().collect();
    (0..chars.len()).find_map(|i| {
        let unescaped = i == 0 || chars[i - 1] != '&';
        match chars.get(i + 1) {
            Some(&key) if chars[i] == '&' && unescaped && key != '&' => {
                Some(key.to_lowercase().collect())
            }
            _ => None,
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DuplicateAccessKey {
    menu_path: String,
    access_key: String,
    first_label: String,
    second_label: String,
}

/// Recursively walk a menu template and collect any duplicate access keys
/// within the same submenu level.
fn find_duplicate_access_keys(items: &[MenuItem], menu_path: &str) -> Vec<DuplicateAccessKey> {
    let mut duplicates = Vec::new();
    let mut seen_keys: Vec<(String, String)> = Vec::new();

    for item in items {
        if item.separator {
            continue;
        }
        if item.visible == Some(false) {
            continue;
        }

        let label = item.label.as_deref();
        if let Some(label) = label
            && let Some(access_key) = get_access_key(label)
        {
            let existing_label = seen_keys
                .iter()
                .find(|(key, _)| *key == access_key)
                .map(|(_, label)| label.clone());
            if let Some(existing_label) = existing_label {
                duplicates.push(DuplicateAccessKey {
                    menu_path: menu_path.to_string(),
                    access_key,
                    first_label: existing_label,
                    second_label: label.to_string(),
                });
            } else {
                seen_keys.push((access_key, label.to_string()));
            }
        }

        if let Some(submenu) = &item.submenu {
            let child_path = match label {
                Some(label) => format!("{menu_path} > {label}"),
                None => menu_path.to_string(),
            };
            duplicates.extend(find_duplicate_access_keys(submenu, &child_path));
        }
    }

    duplicates
}

// GHD: unit/main-process/menu-test.ts › main-process menu › getAccessKey handles escaped ampersands › does not treat && as an access key prefix
#[test]
fn does_not_treat_double_ampersand_as_an_access_key_prefix() {
    // "Save && Upload" has a literal ampersand, no access key
    assert_eq!(get_access_key("Save && Upload"), None);
}

// GHD: unit/main-process/menu-test.ts › main-process menu › getAccessKey handles escaped ampersands › does not treat && at start of word as access key
#[test]
fn does_not_treat_double_ampersand_at_start_of_word_as_access_key() {
    // "Ben&&Jerrys" has a literal ampersand, no access key
    assert_eq!(get_access_key("Ben&&Jerrys"), None);
}

// GHD: unit/main-process/menu-test.ts › main-process menu › getAccessKey handles escaped ampersands › extracts access key after escaped ampersand
#[test]
fn extracts_access_key_after_escaped_ampersand() {
    // "Save && &Upload" has a literal ampersand AND an access key 'u'
    assert_eq!(get_access_key("Save && &Upload").as_deref(), Some("u"));
}

// GHD: unit/main-process/menu-test.ts › main-process menu › getAccessKey handles escaped ampersands › extracts normal access key correctly
#[test]
fn extracts_normal_access_key_correctly() {
    assert_eq!(get_access_key("&File").as_deref(), Some("f"));
    assert_eq!(get_access_key("E&xit").as_deref(), Some("x"));
}

// GHD: unit/main-process/menu-test.ts › main-process menu › buildDefaultMenuTemplate › has no duplicate access keys for any combination of label-affecting parameters
#[test]
#[ignore = "ghd: missing: buildDefaultMenuTemplate's counterpart menus::install is in the binary crate (crates/corvene/src/menus.rs), unreachable"]
fn has_no_duplicate_access_keys_for_any_combination_of_label_affecting_parameters() {
    // The boolean parameters that affect which labels (and therefore access
    // keys) appear in the menu. We generate all 2^N combinations to ensure no
    // state produces a duplicate access key in any submenu.
    type Setter = fn(&mut MenuLabelsEvent, bool);
    let variant_keys: [Setter; 7] = [
        |p, v| p.is_stashed_changes_visible = v,
        |p, v| p.is_changes_filter_visible = v,
        |p, v| p.has_current_pull_request = v,
        |p, v| p.ask_for_confirmation_on_repository_removal = v,
        |p, v| p.ask_for_confirmation_when_stashing_all_changes = v,
        |p, v| p.is_force_push_for_current_repository = v,
        |p, v| p.ask_for_confirmation_on_force_push = v,
    ];

    let base_params = MenuLabelsEvent {
        selected_shell: None,
        selected_external_editor: None,
        ask_for_confirmation_on_force_push: false,
        ask_for_confirmation_on_repository_removal: false,
        ..Default::default()
    };

    let combination_count = 1u32 << variant_keys.len();

    for bits in 0..combination_count {
        let mut params = base_params.clone();
        for (i, set) in variant_keys.iter().enumerate() {
            set(&mut params, bits & (1 << i) != 0);
        }

        let template = build_default_menu_template(&params);
        let duplicates = find_duplicate_access_keys(&template, "root");

        assert_eq!(
            duplicates,
            Vec::<DuplicateAccessKey>::new(),
            "Duplicate access keys found with params {params:?}: {duplicates:?}"
        );
    }
}
