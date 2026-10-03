//! External editor detection on Windows - GHD `lib/editors/win32.ts` (the
//! table of uninstall registry keys, display names and publishers is copied
//! verbatim). An editor is installed when one of its uninstall keys exists
//! with the expected display name and publisher, and the executable the key
//! leads to is there.
//!
//! Flag `extra-editors` adds editors without uninstall keys GHD knows:
//! Microsoft Edit (`edit.exe` on the PATH, started in a console of its own)
//! and gVim (`gvim.exe` on the PATH or in `%ProgramFiles%\Vim\vim*`).
//!
//! Deviation (flag `jetbrains-64bit-hive`): JetBrains IDEs are also looked
//! up under the 64-bit machine uninstall key, where current installers
//! register them; GHD's `registryKeysForJetBrainsIDE` only checks the
//! 32-bit (WOW6432Node) and user keys.

use std::path::{Path, PathBuf};

use windows_registry::{CURRENT_USER, Key, LOCAL_MACHINE};

use super::FoundEditor;

const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
const WOW64_UNINSTALL: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

/// Where an uninstall key is (GHD `CurrentUserUninstallKey`,
/// `LocalMachineUninstallKey`, `Wow64LocalMachineUninstallKey`).
#[derive(Clone, Copy)]
enum Hive {
    User,
    Machine,
    Wow64,
}
use Hive::{Machine, User, Wow64};

impl Hive {
    fn open(self, sub_key: &str) -> Option<Key> {
        let (root, parent) = match self {
            User => (CURRENT_USER, UNINSTALL),
            Machine => (LOCAL_MACHINE, UNINSTALL),
            Wow64 => (LOCAL_MACHINE, WOW64_UNINSTALL),
        };
        root.open(format!(r"{parent}\{sub_key}")).ok()
    }
}

enum Keys {
    List(&'static [(Hive, &'static str)]),
    /// GHD `registryKeysForJetBrainsIDE(product)`
    JetBrains(&'static str),
}

/// GHD `WindowsExternalEditorPathInfo`: the registry value with the
/// location, and the executables relative to it.
enum Location {
    Install(&'static [&'static str]),
    UninstallString(&'static [&'static str]),
    DisplayIcon,
}

struct Editor {
    name: &'static str,
    keys: Keys,
    location: Location,
    display_name_prefixes: &'static [&'static str],
    publishers: &'static [&'static str],
}

const JETBRAINS: &[&str] = &["JetBrains s.r.o."];

/// GHD `executableShimPathsForJetBrainsIDE` is `bin\<name>64.exe`, then
/// `bin\<name>.exe`.
const fn jetbrains(
    name: &'static str,
    product: &'static str,
    shims: &'static [&'static str],
    prefix: &'static [&'static str],
) -> Editor {
    Editor {
        name,
        keys: Keys::JetBrains(product),
        location: Location::Install(shims),
        display_name_prefixes: prefix,
        publishers: JETBRAINS,
    }
}

/// GHD's `editors`, in its order.
const TABLE: &[Editor] = &[
    Editor {
        name: "Visual Studio Code",
        keys: Keys::List(&[
            (User, "{771FD6B0-FA20-440A-A002-3B3BAC16DC50}_is1"),
            (User, "{D628A17A-9713-46BF-8D57-E671B46A741E}_is1"),
            (User, "{D9E514E7-1A56-452D-9337-2990C0DC4310}_is1"),
            (Machine, "{EA457B21-F73E-494C-ACAB-524FDE069978}_is1"),
            (Wow64, "{F8A2A208-72B3-4D61-95FC-8A65D340689B}_is1"),
            (Machine, "{A5270FC5-65AD-483E-AC30-2C276B63D0AC}_is1"),
        ]),
        location: Location::Install(&["code.exe"]),
        display_name_prefixes: &["Microsoft Visual Studio Code"],
        publishers: &["Microsoft Corporation"],
    },
    Editor {
        name: "Visual Studio Code (Insiders)",
        keys: Keys::List(&[
            (User, "{217B4C08-948D-4276-BFBB-BEE930AE5A2C}_is1"),
            (User, "{26F4A15E-E392-4887-8C09-7BC55712FD5B}_is1"),
            (User, "{69BD8F7B-65EB-4C6F-A14E-44CFA83712C0}_is1"),
            (Machine, "{1287CAD5-7C8D-410D-88B9-0D1EE4A83FF2}_is1"),
            (Wow64, "{C26E74D1-022E-4238-8B9D-1E7564A36CC9}_is1"),
            (Machine, "{0AEDB616-9614-463B-97D7-119DD86CCA64}_is1"),
        ]),
        location: Location::Install(&["Code - Insiders.exe"]),
        display_name_prefixes: &["Microsoft Visual Studio Code Insiders"],
        publishers: &["Microsoft Corporation"],
    },
    Editor {
        name: "VSCodium",
        keys: Keys::List(&[
            (User, "{2E1F05D1-C245-4562-81EE-28188DB6FD17}_is1"),
            (User, "{0FD05EB4-651E-4E78-A062-515204B47A3A}_is1"),
            (User, "{57FD70A5-1B8D-4875-9F40-C5553F094828}_is1"),
            (Machine, "{88DA3577-054F-4CA1-8122-7D820494CFFB}_is1"),
            (Wow64, "{763CBF88-25C6-4B10-952F-326AE657F16B}_is1"),
            (Machine, "{67DEE444-3D04-4258-B92A-BC1F0FF2CAE4}_is1"),
            (User, "{C6065F05-9603-4FC4-8101-B9781A25D88E}}_is1"),
            (User, "{3AEBF0C8-F733-4AD4-BADE-FDB816D53D7B}_is1"),
            (Machine, "{D77B7E06-80BA-4137-BCF4-654B95CCEBC5}_is1"),
            (Wow64, "{E34003BB-9E10-4501-8C11-BE3FAA83F23F}_is1"),
            (Machine, "{D1ACE434-89C5-48D1-88D3-E2991DF85475}_is1"),
        ]),
        location: Location::Install(&["VSCodium.exe"]),
        display_name_prefixes: &["VSCodium"],
        publishers: &["VSCodium", "Microsoft Corporation"],
    },
    Editor {
        name: "VSCodium (Insiders)",
        keys: Keys::List(&[
            (User, "{20F79D0D-A9AC-4220-9A81-CE675FFB6B41}_is1"),
            (User, "{ED2E5618-3E7E-4888-BF3C-A6CCC84F586F}_is1"),
            (User, "{2E362F92-14EA-455A-9ABD-3E656BBBFE71}_is1"),
            (Machine, "{B2E0DDB2-120E-4D34-9F7E-8C688FF839A2}_is1"),
            (Wow64, "{EF35BB36-FA7E-4BB9-B7DA-D1E09F2DA9C9}_is1"),
            (Machine, "{44721278-64C6-4513-BC45-D48E07830599}_is1"),
        ]),
        location: Location::Install(&["VSCodium - Insiders.exe"]),
        display_name_prefixes: &["VSCodium Insiders", "VSCodium (Insiders)"],
        publishers: &["VSCodium"],
    },
    Editor {
        name: "Sublime Text",
        keys: Keys::List(&[
            (Machine, "Sublime Text_is1"),
            (Machine, "Sublime Text 3_is1"),
        ]),
        location: Location::Install(&["subl.exe"]),
        display_name_prefixes: &["Sublime Text"],
        publishers: &["Sublime HQ Pty Ltd"],
    },
    Editor {
        name: "Brackets",
        keys: Keys::List(&[(Wow64, "{4F3B6E8C-401B-4EDE-A423-6481C239D6FF}")]),
        location: Location::Install(&["Brackets.exe"]),
        display_name_prefixes: &["Brackets"],
        publishers: &["brackets.io"],
    },
    Editor {
        name: "ColdFusion Builder",
        keys: Keys::List(&[
            (Machine, "Adobe ColdFusion Builder 3_is1"),
            (Machine, "Adobe ColdFusion Builder 2016"),
        ]),
        location: Location::Install(&["CFBuilder.exe"]),
        display_name_prefixes: &["Adobe ColdFusion Builder"],
        publishers: &["Adobe Systems Incorporated"],
    },
    Editor {
        name: "Typora",
        keys: Keys::List(&[
            (Machine, "{37771A20-7167-44C0-B322-FD3E54C56156}_is1"),
            (Wow64, "{37771A20-7167-44C0-B322-FD3E54C56156}_is1"),
        ]),
        location: Location::Install(&["typora.exe"]),
        display_name_prefixes: &["Typora"],
        publishers: &["typora.io"],
    },
    Editor {
        name: "SlickEdit",
        keys: Keys::List(&[
            (Machine, "{18406187-F49E-4822-CAF2-1D25C0C83BA2}"),
            (Wow64, "{18006187-F49E-4822-CAF2-1D25C0C83BA2}"),
            (Machine, "{18606187-F49E-4822-CAF2-1D25C0C83BA2}"),
            (Wow64, "{18206187-F49E-4822-CAF2-1D25C0C83BA2}"),
            (Machine, "{15406187-F49E-4822-CAF2-1D25C0C83BA2}"),
            (Wow64, "{15006187-F49E-4822-CAF2-1D25C0C83BA2}"),
            (Machine, "{10C06187-F49E-4822-CAF2-1D25C0C83BA2}"),
            (Machine, "{10406187-F49E-4822-CAF2-1D25C0C83BA2}"),
            (Machine, "{0DC06187-F49E-4822-CAF2-1D25C0C83BA2}"),
            (Machine, "{0D406187-F49E-4822-CAF2-1D25C0C83BA2}"),
            (Machine, "{7CC0E567-ACD6-41E8-95DA-154CEEDB0A18}"),
        ]),
        location: Location::Install(&[r"win\vs.exe"]),
        display_name_prefixes: &["SlickEdit"],
        publishers: &["SlickEdit Inc."],
    },
    Editor {
        name: "Aptana Studio 3",
        keys: Keys::List(&[(Wow64, "{2D6C1116-78C6-469C-9923-3E549218773F}")]),
        location: Location::Install(&["AptanaStudio3.exe"]),
        display_name_prefixes: &["Aptana Studio"],
        publishers: &["Appcelerator"],
    },
    jetbrains(
        "JetBrains Webstorm",
        "WebStorm",
        &[r"bin\webstorm64.exe", r"bin\webstorm.exe"],
        &["WebStorm"],
    ),
    jetbrains(
        "JetBrains PhpStorm",
        "PhpStorm",
        &[r"bin\phpstorm64.exe", r"bin\phpstorm.exe"],
        &["PhpStorm"],
    ),
    Editor {
        name: "Android Studio",
        keys: Keys::List(&[(Machine, "Android Studio")]),
        location: Location::UninstallString(&[r"..\bin\studio64.exe", r"..\bin\studio.exe"]),
        display_name_prefixes: &["Android Studio"],
        publishers: &["Google LLC"],
    },
    Editor {
        name: "Notepad++",
        keys: Keys::List(&[(Machine, "Notepad++"), (Wow64, "Notepad++")]),
        location: Location::DisplayIcon,
        display_name_prefixes: &["Notepad++"],
        publishers: &["Notepad++ Team"],
    },
    jetbrains(
        "JetBrains Rider",
        "JetBrains Rider",
        &[r"bin\rider64.exe", r"bin\rider.exe"],
        &["JetBrains Rider"],
    ),
    Editor {
        name: "RStudio",
        keys: Keys::List(&[(User, "RStudio"), (Machine, "RStudio"), (Wow64, "RStudio")]),
        location: Location::DisplayIcon,
        display_name_prefixes: &["RStudio"],
        publishers: &["RStudio", "Posit Software"],
    },
    jetbrains(
        "JetBrains IntelliJ Idea",
        "IntelliJ IDEA",
        &[r"bin\idea64.exe", r"bin\idea.exe"],
        &["IntelliJ IDEA "],
    ),
    jetbrains(
        "JetBrains IntelliJ Idea Community Edition",
        "IntelliJ IDEA Community Edition",
        &[r"bin\idea64.exe", r"bin\idea.exe"],
        &["IntelliJ IDEA Community Edition "],
    ),
    jetbrains(
        "JetBrains PyCharm",
        "PyCharm",
        &[r"bin\pycharm64.exe", r"bin\pycharm.exe"],
        &["PyCharm "],
    ),
    jetbrains(
        "JetBrains PyCharm Community Edition",
        "PyCharm Community Edition",
        &[r"bin\pycharm64.exe", r"bin\pycharm.exe"],
        &["PyCharm Community Edition"],
    ),
    jetbrains(
        "JetBrains CLion",
        "CLion",
        &[r"bin\clion64.exe", r"bin\clion.exe"],
        &["CLion "],
    ),
    jetbrains(
        "JetBrains RubyMine",
        "RubyMine",
        &[r"bin\rubymine64.exe", r"bin\rubymine.exe"],
        &["RubyMine "],
    ),
    jetbrains(
        "JetBrains GoLand",
        "GoLand",
        &[r"bin\goland64.exe", r"bin\goland.exe"],
        &["GoLand "],
    ),
    Editor {
        name: "JetBrains Fleet",
        keys: Keys::List(&[(Machine, "Fleet")]),
        location: Location::DisplayIcon,
        display_name_prefixes: &["Fleet "],
        publishers: JETBRAINS,
    },
    jetbrains(
        "JetBrains DataSpell",
        "DataSpell",
        &[r"bin\dataspell64.exe", r"bin\dataspell.exe"],
        &["DataSpell "],
    ),
    jetbrains(
        "JetBrains RustRover",
        "RustRover",
        &[r"bin\rustrover64.exe", r"bin\rustrover.exe"],
        &["RustRover "],
    ),
    Editor {
        name: "Pulsar",
        keys: Keys::List(&[
            (User, "0949b555-c22c-56b7-873a-a960bdefa81f"),
            (Machine, "0949b555-c22c-56b7-873a-a960bdefa81f"),
        ]),
        location: Location::Install(&[r"..\pulsar\Pulsar.exe"]),
        display_name_prefixes: &["Pulsar"],
        publishers: &["Pulsar-Edit"],
    },
    Editor {
        name: "Cursor",
        keys: Keys::List(&[
            (User, "62625861-8486-5be9-9e46-1da50df5f8ff"),
            (User, "{DADADADA-ADAD-ADAD-ADAD-ADADADADADAD}}_is1"),
            // ARM64 version of Cursor
            (User, "{DBDBDBDB-BDBD-BDBD-BDBD-BDBDBDBDBDBD}}_is1"),
        ]),
        location: Location::DisplayIcon,
        display_name_prefixes: &["Cursor", "Cursor (User)"],
        publishers: &["Cursor AI, Inc.", "Anysphere"],
    },
    Editor {
        name: "Windsurf",
        keys: Keys::List(&[(User, "{5A8B7D94-9B5F-4D1F-93FC-5609F7159349}_is1")]),
        location: Location::DisplayIcon,
        display_name_prefixes: &["Windsurf", "Windsurf (User)"],
        publishers: &["Codeium"],
    },
    Editor {
        name: "Zed",
        keys: Keys::List(&[(User, "{2DB0DA96-CA55-49BB-AF4F-64AF36A86712}_is1")]),
        location: Location::DisplayIcon,
        display_name_prefixes: &["Zed"],
        publishers: &["Zed Industries"],
    },
];

/// The uninstall key names of a JetBrains product for the last two years,
/// newest first: up to 5 major and 5 minor releases a year
/// (`<product> <year>.<major>[.<minor>]`), each under the 32-bit machine key
/// and the user key (and with `machine_hive` the 64-bit machine key).
fn jetbrains_keys(product: &str, this_year: i32, machine_hive: bool) -> Vec<(Hive, String)> {
    let mut keys = Vec::new();
    for year in this_year - 2..=this_year {
        for major in 1..=5 {
            for minor in 0..=5 {
                let mut key = format!("{product} {year}.{major}");
                if minor > 0 {
                    key.push_str(&format!(".{minor}"));
                }
                if machine_hive {
                    keys.push((Machine, key.clone()));
                }
                keys.push((Wow64, key.clone()));
                keys.push((User, key));
            }
        }
    }
    keys.reverse();
    keys
}

fn this_year() -> i32 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // the mean Gregorian year; exact around New Year is not needed for a
    // three-year window
    1970 + (secs / 31_556_952) as i32
}

/// GHD `getCleanInstallLocationFromDisplayIcon`: `"C:\Path\app.exe",0` →
/// `C:\Path\app.exe`.
fn clean_display_icon(value: &str) -> String {
    value.split(',').next().unwrap_or(value).replace('"', "")
}

/// GHD `findApplication` for one uninstall key.
fn executable_from(
    key: &Key,
    location: &Location,
    display_name_prefixes: &[&str],
    publishers: &[&str],
) -> Option<PathBuf> {
    let value = |name: &str| key.get_string(name).unwrap_or_default();
    let display_name = value("DisplayName");
    if !display_name_prefixes
        .iter()
        .any(|prefix| display_name.starts_with(prefix))
        || !publishers.contains(&value("Publisher").as_str())
    {
        return None;
    }
    let candidates: Vec<PathBuf> = match location {
        Location::DisplayIcon => vec![PathBuf::from(clean_display_icon(&value("DisplayIcon")))],
        Location::Install(shims) => {
            let dir = value("InstallLocation");
            shims
                .iter()
                .map(|shim| Path::new(&dir).join(shim))
                .collect()
        }
        Location::UninstallString(shims) => {
            let dir = value("UninstallString");
            shims
                .iter()
                .map(|shim| Path::new(&dir).join(shim))
                .collect()
        }
    };
    candidates.into_iter().find(|path| path.is_file())
}

fn find(editor: &Editor, year: i32, machine_hive: bool) -> Option<PathBuf> {
    let from = |hive: Hive, sub_key: &str| {
        executable_from(
            &hive.open(sub_key)?,
            &editor.location,
            editor.display_name_prefixes,
            editor.publishers,
        )
    };
    match &editor.keys {
        Keys::List(keys) => keys.iter().find_map(|(hive, key)| from(*hive, key)),
        Keys::JetBrains(product) => jetbrains_keys(product, year, machine_hive)
            .iter()
            .find_map(|(hive, key)| from(*hive, key)),
    }
}

/// GHD `getJetBrainsToolboxEditors`: the IDEs JetBrains Toolbox installed,
/// which register as `JetBrains Toolbox (<product>) …` under the user's
/// uninstall keys.
fn toolbox_editors() -> Vec<FoundEditor> {
    let mut out = Vec::new();
    for parent in [UNINSTALL, WOW64_UNINSTALL] {
        let Ok(uninstall) = CURRENT_USER.open(parent) else {
            continue;
        };
        let Ok(names) = uninstall.keys() else {
            continue;
        };
        for sub_key in names {
            // GHD's `/^JetBrains Toolbox \(.*\)/`: the name is the match
            let Some(name) = sub_key
                .strip_prefix("JetBrains Toolbox (")
                .and_then(|rest| rest.rfind(')'))
                .map(|end| &sub_key["JetBrains Toolbox (".len() + end + 1..])
                .map(|after| &sub_key[..sub_key.len() - after.len()])
            else {
                continue;
            };
            let Ok(key) = uninstall.open(&sub_key) else {
                continue;
            };
            let display_name = key.get_string("DisplayName").unwrap_or_default();
            if display_name.is_empty() {
                continue;
            }
            if let Some(path) =
                executable_from(&key, &Location::DisplayIcon, &[&display_name], JETBRAINS)
            {
                out.push(FoundEditor {
                    name: name.to_string(),
                    bundle_id: String::new(),
                    path,
                });
            }
        }
    }
    out
}

/// Flag `extra-editors`: Microsoft Edit, a console program.
const MICROSOFT_EDIT: &str = "Microsoft Edit";

/// The first `exe` in the PATH's folders (like `findGitOnPath`).
fn on_path(exe: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(exe))
        .find(|candidate| candidate.is_file())
}

/// gVim's installer puts it in `%ProgramFiles%\Vim\vim<version>`; the
/// newest version wins.
fn gvim() -> Option<PathBuf> {
    on_path("gvim.exe").or_else(|| {
        let vim = PathBuf::from(std::env::var_os("ProgramFiles")?).join("Vim");
        let mut found: Vec<PathBuf> = std::fs::read_dir(vim)
            .ok()?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|dir| {
                dir.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("vim"))
            })
            .map(|dir| dir.join("gvim.exe"))
            .filter(|exe| exe.is_file())
            .collect();
        found.sort();
        found.pop()
    })
}

/// Flag `extra-editors`: the editors found outside the uninstall keys.
fn extra_editors() -> Vec<FoundEditor> {
    [(MICROSOFT_EDIT, on_path("edit.exe")), ("gVim", gvim())]
        .into_iter()
        .filter_map(|(name, path)| {
            Some(FoundEditor {
                name: name.to_string(),
                bundle_id: String::new(),
                path: path?,
            })
        })
        .collect()
}

/// Whether `editor` is a console program that needs a console window of its
/// own (Microsoft Edit).
pub fn needs_console(editor: &FoundEditor) -> bool {
    editor.name == MICROSOFT_EDIT
}

/// Start a console editor in a new console window. Its standard handles
/// are left alone so that it attaches to that console.
pub fn spawn_in_console(program: &Path, args: &[&str]) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new(program)
        .args(args)
        .creation_flags(crate::windows::CREATE_NEW_CONSOLE)
        .spawn()
        .map(drop)
}

/// GHD `getAvailableEditors`: the table's installed editors, then the
/// JetBrains Toolbox ones (then, with `extras`, Microsoft Edit and gVim).
/// `jetbrains_machine_hive`: flag `jetbrains-64bit-hive`.
pub fn available(extras: bool, jetbrains_machine_hive: bool) -> Vec<FoundEditor> {
    let year = this_year();
    let mut out: Vec<FoundEditor> = TABLE
        .iter()
        .filter_map(|editor| {
            find(editor, year, jetbrains_machine_hive).map(|path| FoundEditor {
                name: editor.name.to_string(),
                bundle_id: String::new(),
                path,
            })
        })
        .collect();
    out.extend(toolbox_editors());
    if extras {
        out.extend(extra_editors());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_icons_lose_their_index_and_quotes() {
        assert_eq!(
            clean_display_icon(r#""C:\Path\app.exe",0"#),
            r"C:\Path\app.exe"
        );
        assert_eq!(clean_display_icon(r"C:\Path\app.exe"), r"C:\Path\app.exe");
    }

    #[test]
    fn jetbrains_keys_are_newest_first() {
        let keys = jetbrains_keys("WebStorm", 2026, false);
        assert_eq!(keys.len(), 3 * 5 * 6 * 2);
        assert_eq!(keys[0].1, "WebStorm 2026.5.5");
        assert_eq!(keys.last().unwrap().1, "WebStorm 2024.1");
        // `jetbrains-64bit-hive`: the 64-bit machine key too
        let keys = jetbrains_keys("WebStorm", 2026, true);
        assert_eq!(keys.len(), 3 * 5 * 6 * 3);
        assert!(matches!(keys.last().unwrap().0, Machine));
    }
}
