//! External editor detection - GHD `lib/editors/darwin.ts` (the bundle
//! identifier table is copied verbatim), `lib/editors/linux.ts` (the path
//! table, likewise) and `lib/editors/launch.ts`.
//!
//! Linux: GHD's relative paths (`.local/share/flatpak/…`, JetBrains
//! Toolbox scripts) and `~/.local/bin/zed` resolve against the home folder;
//! GHD passes them to `pathExists` as they are, which resolves against the
//! process's working directory and only finds them when that is home.
//!
//! `launch_at_line` is Corvene's (flag `diff-open-in-editor-at-line`): VS Code
//! and its forks, Sublime Text and Zed open a file at a line through the
//! command line tool in their bundle; other editors just open the file.
//! `EXTRA_EDITORS` (flag `extra-editors`) adds editors GHD does not list.
//! Deviation: [`code_workspace_file`] lets VS Code and its forks open a
//! repository's only `*.code-workspace` file (`509-vscode-workspace-file`).

use std::path::{Path, PathBuf};

// Android opens an editor with an intent, not a process
#[cfg(not(target_os = "android"))]
use crate::apps;

#[cfg(windows)]
#[path = "editors_windows.rs"]
mod windows_editors;

/// Friendly name + bundle identifiers, in GHD's order (the first installed
/// editor is the default when none is selected in Settings).
#[cfg(target_os = "macos")]
const EDITORS: &[(&str, &[&str])] = &[
    ("Atom", &["com.github.atom"]),
    ("Aptana Studio", &["aptana.studio"]),
    ("Eclipse IDE for Java Developers", &["epp.package.java"]),
    (
        "Eclipse IDE for Enterprise Java and Web Developers",
        &["epp.package.jee"],
    ),
    ("Eclipse IDE for C/C++ Developers", &["epp.package.cpp"]),
    (
        "Eclipse IDE for Eclipse Committers",
        &["epp.package.committers"],
    ),
    (
        "Eclipse IDE for Embedded C/C++ Developers",
        &["epp.package.embedcpp"],
    ),
    ("Eclipse IDE for PHP Developers", &["epp.package.php"]),
    (
        "Eclipse IDE for Java and DSL Developers",
        &["epp.package.dsl"],
    ),
    (
        "Eclipse IDE for RCP and RAP Developers",
        &["epp.package.rcp"],
    ),
    ("Eclipse Modeling Tools", &["epp.package.modeling"]),
    (
        "Eclipse IDE for Scientific Computing",
        &["epp.package.parallel"],
    ),
    ("Eclipse IDE for Scout Developers", &["epp.package.scout"]),
    ("MacVim", &["org.vim.MacVim"]),
    ("Neovide", &["com.neovide.neovide"]),
    ("VimR", &["com.qvacua.VimR"]),
    ("Visual Studio Code", &["com.microsoft.VSCode"]),
    (
        "Visual Studio Code (Insiders)",
        &["com.microsoft.VSCodeInsiders"],
    ),
    ("VSCodium", &["com.visualstudio.code.oss", "com.vscodium"]),
    (
        "Sublime Text",
        &[
            "com.sublimetext.4",
            "com.sublimetext.3",
            "com.sublimetext.2",
        ],
    ),
    ("BBEdit", &["com.barebones.bbedit"]),
    ("PhpStorm", &["com.jetbrains.PhpStorm"]),
    ("PyCharm", &["com.jetbrains.PyCharm"]),
    ("PyCharm Community Edition", &["com.jetbrains.pycharm.ce"]),
    ("DataSpell", &["com.jetbrains.DataSpell"]),
    ("RubyMine", &["com.jetbrains.RubyMine"]),
    ("RustRover", &["com.jetbrains.RustRover"]),
    ("RStudio", &["org.rstudio.RStudio", "com.rstudio.desktop"]),
    ("TextMate", &["com.macromates.TextMate"]),
    ("Brackets", &["io.brackets.appshell"]),
    ("WebStorm", &["com.jetbrains.WebStorm"]),
    ("CLion", &["com.jetbrains.CLion"]),
    ("Typora", &["abnerworks.Typora"]),
    ("CodeRunner", &["com.krill.CodeRunner"]),
    (
        "SlickEdit",
        &[
            "com.slickedit.SlickEditPro2018",
            "com.slickedit.SlickEditPro2017",
            "com.slickedit.SlickEditPro2016",
            "com.slickedit.SlickEditPro2015",
        ],
    ),
    ("IntelliJ", &["com.jetbrains.intellij"]),
    ("IntelliJ Community Edition", &["com.jetbrains.intellij.ce"]),
    ("Xcode", &["com.apple.dt.Xcode"]),
    ("GoLand", &["com.jetbrains.goland"]),
    ("Android Studio", &["com.google.android.studio"]),
    ("Rider", &["com.jetbrains.rider"]),
    ("Nova", &["com.panic.Nova"]),
    ("Emacs", &["org.gnu.Emacs"]),
    ("Lite XL", &["com.lite-xl"]),
    ("Fleet", &["Fleet.app"]),
    ("Pulsar", &["dev.pulsar-edit.pulsar"]),
    ("Zed", &["dev.zed.Zed"]),
    ("Zed (Preview)", &["dev.zed.Zed-Preview"]),
    ("Cursor", &["com.todesktop.230313mzl4w4u92"]),
    ("Windsurf", &["com.exafunction.windsurf"]),
];

/// Friendly name + executable paths (GHD `lib/editors/linux.ts`), in GHD's
/// order; the first existing path wins.
#[cfg(not(target_os = "macos"))]
const EDITORS: &[(&str, &[&str])] = &[
    ("Atom", &["/snap/bin/atom", "/usr/bin/atom"]),
    ("Neovim", &["/usr/bin/nvim"]),
    ("Neovim-Qt", &["/usr/bin/nvim-qt"]),
    ("Neovide", &["/usr/bin/neovide"]),
    ("gVim", &["/usr/bin/gvim"]),
    (
        "Visual Studio Code",
        &[
            "/usr/share/code/bin/code",
            "/snap/bin/code",
            "/usr/bin/code",
            "/mnt/c/Program Files/Microsoft VS Code/bin/code",
            "/var/lib/flatpak/app/com.visualstudio.code/current/active/export/bin/com.visualstudio.code",
            ".local/share/flatpak/app/com.visualstudio.code/current/active/export/bin/com.visualstudio.code",
        ],
    ),
    (
        "Visual Studio Code (Insiders)",
        &[
            "/snap/bin/code-insiders",
            "/usr/bin/code-insiders",
            "/var/lib/flatpak/app/com.visualstudio.code.insiders/current/active/export/bin/com.visualstudio.code.insiders",
            ".local/share/flatpak/app/com.visualstudio.code.insiders/current/active/export/bin/com.visualstudio.code.insiders",
        ],
    ),
    (
        "VSCodium",
        &[
            "/usr/bin/codium",
            "/var/lib/flatpak/app/com.vscodium.codium/current/active/export/bin/com.vscodium.codium",
            "/usr/share/vscodium-bin/bin/codium",
            ".local/share/flatpak/app/com.vscodium.codium/current/active/export/bin/com.vscodium.codium",
            "/snap/bin/codium",
        ],
    ),
    ("VSCodium (Insiders)", &["/usr/bin/codium-insiders"]),
    ("Sublime Text", &["/usr/bin/subl"]),
    ("Typora", &["/usr/bin/typora"]),
    (
        "SlickEdit",
        &[
            "/opt/slickedit-pro2018/bin/vs",
            "/opt/slickedit-pro2017/bin/vs",
            "/opt/slickedit-pro2016/bin/vs",
            "/opt/slickedit-pro2015/bin/vs",
        ],
    ),
    // elementary OS's code editor
    ("Code", &["/usr/bin/io.elementary.code"]),
    ("Lite XL", &["/usr/bin/lite-xl"]),
    (
        "JetBrains PhpStorm",
        &[
            "/snap/bin/phpstorm",
            ".local/share/JetBrains/Toolbox/scripts/PhpStorm",
        ],
    ),
    (
        "JetBrains WebStorm",
        &[
            "/snap/bin/webstorm",
            ".local/share/JetBrains/Toolbox/scripts/webstorm",
        ],
    ),
    (
        "IntelliJ IDEA",
        &[
            "/snap/bin/idea",
            ".local/share/JetBrains/Toolbox/scripts/idea",
        ],
    ),
    (
        "IntelliJ IDEA Ultimate Edition",
        &[
            "/snap/bin/intellij-idea-ultimate",
            ".local/share/JetBrains/Toolbox/scripts/intellij-idea-ultimate",
        ],
    ),
    (
        "JetBrains Goland",
        &[
            "/snap/bin/goland",
            ".local/share/JetBrains/Toolbox/scripts/goland",
        ],
    ),
    (
        "JetBrains CLion",
        &[
            "/snap/bin/clion",
            ".local/share/JetBrains/Toolbox/scripts/clion1",
        ],
    ),
    (
        "JetBrains Rider",
        &[
            "/snap/bin/rider",
            ".local/share/JetBrains/Toolbox/scripts/rider",
        ],
    ),
    (
        "JetBrains RubyMine",
        &[
            "/snap/bin/rubymine",
            ".local/share/JetBrains/Toolbox/scripts/rubymine",
        ],
    ),
    (
        "JetBrains PyCharm",
        &[
            "/snap/bin/pycharm",
            "/snap/bin/pycharm-professional",
            ".local/share/JetBrains/Toolbox/scripts/pycharm",
        ],
    ),
    (
        "JetBrains RustRover",
        &[
            "/snap/bin/rustrover",
            ".local/share/JetBrains/Toolbox/scripts/rustrover",
        ],
    ),
    (
        "Android Studio",
        &[
            "/snap/bin/studio",
            ".local/share/JetBrains/Toolbox/scripts/studio",
        ],
    ),
    (
        "Emacs",
        &["/snap/bin/emacs", "/usr/local/bin/emacs", "/usr/bin/emacs"],
    ),
    ("Kate", &["/usr/bin/kate"]),
    ("GEdit", &["/usr/bin/gedit"]),
    ("GNOME Text Editor", &["/usr/bin/gnome-text-editor"]),
    ("GNOME Builder", &["/usr/bin/gnome-builder"]),
    ("Notepadqq", &["/usr/bin/notepadqq"]),
    ("Mousepad", &["/usr/bin/mousepad"]),
    ("Pulsar", &["/usr/bin/pulsar"]),
    ("Pluma", &["/usr/bin/pluma"]),
    (
        "Zed",
        &[
            "/usr/bin/zedit",
            "/usr/bin/zeditor",
            "/usr/bin/zed-editor",
            "~/.local/bin/zed",
            "/usr/bin/zed",
        ],
    ),
];

/// Editors GHD 3.6.6 does not know, in the same shape; detected only with
/// flag `extra-editors` and listed after GHD's.
#[cfg(target_os = "macos")]
const EXTRA_EDITORS: &[(&str, &[&str])] = &[
    // desktop/desktop#22922, bundle id from desktop/desktop#21417
    ("Antigravity", &["com.google.antigravity"]),
];
#[cfg(not(target_os = "macos"))]
const EXTRA_EDITORS: &[(&str, &[&str])] = &[
    // desktop/desktop#22922
    ("Antigravity", &["/usr/bin/antigravity"]),
    ("Cursor", &["/usr/bin/cursor", "/opt/cursor/cursor"]),
    ("Windsurf", &["/usr/bin/windsurf"]),
];

/// GHD's Linux paths: absolute, `~/…`, or relative to the home folder.
#[cfg(not(target_os = "macos"))]
fn resolve_linux_path(path: &str) -> PathBuf {
    let home = || dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    if let Some(rest) = path.strip_prefix("~/") {
        home().join(rest)
    } else if path.starts_with('/') {
        PathBuf::from(path)
    } else {
        home().join(path)
    }
}

/// The first of `paths` that exists (GHD `getAvailablePath`).
#[cfg(not(target_os = "macos"))]
fn first_existing(paths: &[&str]) -> Option<(String, PathBuf)> {
    paths.iter().find_map(|p| {
        let path = resolve_linux_path(p);
        path.exists().then(|| ((*p).to_string(), path))
    })
}

/// Editors that open VS Code `.code-workspace` files (`509-vscode-workspace-file`).
const CODE_WORKSPACE_EDITORS: &[&str] = &[
    "Visual Studio Code",
    "Visual Studio Code (Insiders)",
    "VSCodium",
    "Cursor",
    "Windsurf",
];

/// Corvene `509-vscode-workspace-file`: the one `*.code-workspace` file at
/// the top of `dir` when `editor` is VS Code or a fork of it; `None` when
/// there is none or several.
pub fn code_workspace_file(editor: &FoundEditor, dir: &Path) -> Option<PathBuf> {
    if !CODE_WORKSPACE_EDITORS.contains(&editor.name.as_str()) {
        return None;
    }
    let mut found = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "code-workspace"));
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

/// GHD `suggestedExternalEditor`.
pub const SUGGESTED_EDITOR_NAME: &str = "Visual Studio Code";
pub const SUGGESTED_EDITOR_URL: &str = "https://code.visualstudio.com";

/// Android: the name of the "editor" that asks the system which application
/// opens the file (an `ACTION_VIEW` intent without a target).
#[cfg(target_os = "android")]
pub const ANDROID_EDITOR_NAME: &str = "Another App";

/// Android: the terminal editors looked for in Termux (program, name). The
/// installed ones are editors like the applications are, named
/// "<name> (Termux)" with [`TERMUX_PREFIX`] and the program as `bundle_id`;
/// they run in a new Termux session through its `RUN_COMMAND` intent, so
/// only for files on shared storage (see `shells::android`).
pub const TERMUX_EDITORS: &[(&str, &str)] = &[
    ("nvim", "Neovim"),
    ("vim", "Vim"),
    ("hx", "Helix"),
    ("micro", "Micro"),
    ("nano", "nano"),
    ("emacs", "Emacs"),
];

/// `bundle_id` of an editor that runs in Termux: this and its program.
pub const TERMUX_PREFIX: &str = "termux:";

/// The "program" of the entry that runs whatever `$EDITOR` names in the
/// user's Termux shell (`vi` when it names nothing). Always offered: it
/// needs no answer from Termux about what is installed.
pub const TERMUX_DEFAULT_EDITOR: &str = "$EDITOR";

/// `text` as one word of a POSIX (or fish) shell command.
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// The program in Termux's `bin`, its arguments and the working directory
/// that open `target` (a folder when `is_dir`) in the Termux editor
/// `program`, at `line` when given.
pub fn termux_command(
    program: &str,
    target: &Path,
    is_dir: bool,
    line: Option<u32>,
) -> (String, Vec<String>, PathBuf) {
    let dir = if is_dir {
        target.to_path_buf()
    } else {
        target
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("/"))
    };
    let file = if is_dir {
        ".".to_string()
    } else {
        target.to_string_lossy().into_owned()
    };
    let mut args = Vec::new();
    match (program, line) {
        // micro and nano take files only: in a folder they start empty
        ("micro" | "nano", _) if is_dir => {}
        ("hx", Some(line)) => args.push(format!("{file}:{line}")),
        (_, Some(line)) => args.extend([format!("+{line}"), file]),
        (_, None) => args.push(file),
    }
    if program != TERMUX_DEFAULT_EDITOR {
        return (program.to_string(), args, dir);
    }
    // `login` starts the user's shell, which knows `$EDITOR` once it has
    // read its startup files (`-i`: `.bashrc` and `.zshrc` are for
    // interactive shells). The inner `sh` keeps the command the same for
    // bash, zsh and fish.
    let words: Vec<String> = args.iter().map(|a| shell_quote(a)).collect();
    let command = format!(
        "exec sh -c 'exec ${{EDITOR:-vi}} \"$@\"' sh {}",
        words.join(" ")
    );
    (
        "login".to_string(),
        vec!["-i".to_string(), "-c".to_string(), command],
        dir,
    )
}

/// Android: the launcher icon (PNG) of the application an editor's
/// `bundle_id` or a package name stands for. `None` elsewhere.
pub fn app_icon(key: &str) -> Option<Vec<u8>> {
    #[cfg(target_os = "android")]
    if !key.is_empty() {
        let key = if key.starts_with(TERMUX_PREFIX) {
            crate::android::TERMUX_PACKAGE
        } else {
            key
        };
        return crate::android::bridge()?.app_icon(key);
    }
    let _ = key;
    None
}

/// GHD `FoundEditor`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundEditor {
    pub name: String,
    /// macOS: the bundle identifier that matched. Linux: the table entry
    /// that matched (GHD's spelling of the path).
    pub bundle_id: String,
    /// macOS: the `.app` bundle. Linux: the executable.
    pub path: PathBuf,
}

/// Every known editor installed on this machine, in table order (then
/// [`EXTRA_EDITORS`] when `extras`). Costs one LaunchServices lookup (or
/// `stat`) per candidate; run it off the main thread.
pub fn available_editors(extras: bool) -> Vec<FoundEditor> {
    // Android has no editor executables to look for. The editors are the
    // applications that open a text file (`bundle_id` is the activity,
    // "package/class"), after one entry that leaves the choice to the
    // system each time ([`launch`]).
    #[cfg(target_os = "android")]
    if let Some(bridge) = crate::android::bridge() {
        let _ = extras;
        let app = |name: String, component: String| FoundEditor {
            name,
            bundle_id: component,
            path: PathBuf::from("/system"),
        };
        let mut editors = vec![app(ANDROID_EDITOR_NAME.to_string(), String::new())];
        editors.extend(
            bridge
                .view_apps()
                .into_iter()
                .map(|(label, component)| app(label, component)),
        );
        if bridge.package_installed(crate::android::TERMUX_PACKAGE) {
            let programs: Vec<&str> = TERMUX_EDITORS.iter().map(|(program, _)| *program).collect();
            let installed = bridge.termux_programs(&programs).unwrap_or_default();
            let termux = |name: &str, program: &str| {
                app(
                    format!("{name} (Termux)"),
                    format!("{TERMUX_PREFIX}{program}"),
                )
            };
            editors.extend(
                TERMUX_EDITORS
                    .iter()
                    .filter(|(program, _)| installed.iter().any(|i| i == program))
                    .map(|(program, name)| termux(name, program)),
            );
            editors.push(termux(TERMUX_DEFAULT_EDITOR, TERMUX_DEFAULT_EDITOR));
        }
        return editors;
    }
    // Windows: GHD's registry lookups, whose table already has the editors
    // `extras` adds elsewhere; `extras` adds Microsoft Edit and gVim there
    #[cfg(windows)]
    if cfg!(windows) {
        return windows_editors::available(extras);
    }
    let extra: &[(&str, &[&str])] = if extras { EXTRA_EDITORS } else { &[] };
    #[cfg(target_os = "macos")]
    let find = apps::first_installed;
    #[cfg(not(target_os = "macos"))]
    let find = first_existing;
    EDITORS
        .iter()
        .chain(extra)
        .filter_map(|(name, ids)| {
            find(ids).map(|(bundle_id, path)| FoundEditor {
                name: (*name).to_string(),
                bundle_id,
                path,
            })
        })
        .collect()
}

/// Why launching failed (GHD `ExternalEditorError` + its metadata).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorError {
    pub message: String,
    /// Offer a link to install the suggested editor.
    pub suggest_default_editor: bool,
    /// Offer to open Settings › Integrations.
    pub open_preferences: bool,
}

/// GHD `findEditorOrDefault`: the named editor, else the first installed one.
pub fn find_editor_or_default<'a>(
    editors: &'a [FoundEditor],
    name: Option<&str>,
) -> Result<Option<&'a FoundEditor>, EditorError> {
    if editors.is_empty() {
        return Ok(None);
    }
    match name {
        Some(name) => editors
            .iter()
            .find(|e| e.name == name)
            .map(Some)
            .ok_or_else(|| EditorError {
                message: format!(
                    "The editor '{name}' could not be found. Please open {SETTINGS_LABEL} and choose an available editor."
                ),
                suggest_default_editor: false,
                open_preferences: true,
            }),
        None => Ok(editors.first()),
    }
}

/// GHD's name for its settings dialog in messages: "Settings" on macOS,
/// "Options" elsewhere (`__DARWIN__ ? 'Settings' : 'Options'`).
pub const SETTINGS_LABEL: &str = if cfg!(target_os = "macos") {
    "Settings"
} else {
    "Options"
};

/// GHD `launchExternalEditor`: `open -a <bundle> <path>` on macOS, the
/// executable with the path elsewhere, detached.
pub fn launch(editor: &FoundEditor, target: &Path) -> Result<(), EditorError> {
    if !editor.path.exists() {
        return Err(EditorError {
            message: format!(
                "Could not find executable for '{}' at path '{}'. Please open {SETTINGS_LABEL} and select an available editor.",
                editor.name,
                editor.path.display()
            ),
            suggest_default_editor: false,
            open_preferences: true,
        });
    }
    #[cfg(target_os = "macos")]
    let launched = apps::open_with_app(&editor.path, target);
    #[cfg(not(any(target_os = "macos", target_os = "android", windows)))]
    let launched = apps::spawn_detached(&editor.path, &[&target.to_string_lossy()]);
    // `extra-editors`: Microsoft Edit needs a console window
    #[cfg(windows)]
    let launched = if windows_editors::needs_console(editor) {
        windows_editors::spawn_in_console(&editor.path, &[&target.to_string_lossy()])
    } else {
        apps::spawn_detached(&editor.path, &[&target.to_string_lossy()])
    };
    #[cfg(target_os = "android")]
    return launch_android(editor, target, None);
    #[cfg(not(target_os = "android"))]
    return launched.map_err(|err| EditorError {
        message: if err.kind() == std::io::ErrorKind::PermissionDenied {
            format!(
                "Corvene doesn't have the proper permissions to start '{}'. Please open {SETTINGS_LABEL} and try another editor.",
                editor.name
            )
        } else {
            format!(
                "Something went wrong while trying to start '{}'. Please open {SETTINGS_LABEL} and try another editor.",
                editor.name
            )
        },
        suggest_default_editor: false,
        open_preferences: true,
    });
}

/// Android: a Termux editor runs in a new Termux session; an application
/// gets the file (and the line, for those that read it) in an intent, and a
/// folder goes to the file manager, because applications take files only.
/// The message says what went wrong: there is no executable to blame.
#[cfg(target_os = "android")]
fn launch_android(
    editor: &FoundEditor,
    target: &Path,
    line: Option<u32>,
) -> Result<(), EditorError> {
    let error = |message: String| EditorError {
        message,
        suggest_default_editor: false,
        open_preferences: true,
    };
    let Some(bridge) = crate::android::bridge() else {
        return Err(error("Corvene is not open.".to_string()));
    };
    if let Some(program) = editor.bundle_id.strip_prefix(TERMUX_PREFIX) {
        if !crate::android::is_shared_storage(target) {
            return Err(error(
                "Termux cannot reach a repository in Corvene's private storage. \
                 Clone or add it on shared storage (a folder under /storage/emulated/0, \
                 with \"All files access\") to work on it in both."
                    .to_string(),
            ));
        }
        let (program, arguments, dir) = termux_command(program, target, target.is_dir(), line);
        return bridge.run_termux(&program, &arguments, &dir).map_err(error);
    }
    if !editor.bundle_id.is_empty() && target.is_file() {
        return bridge
            .view_path_with(target, &editor.bundle_id, line)
            .map_err(error);
    }
    bridge.view_path(target).map_err(error)
}

/// Android: the applications known to show the line an intent names
/// (Markor's `EXTRA_FILE_LINE_NUMBER`), by package.
#[cfg(target_os = "android")]
const LINE_APPS: &[&str] = &["net.gsantner.markor"];

/// How an editor's bundled command line tool opens a file at a line (not in
/// GHD, which only opens files; desktop/desktop#14476).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineArgs {
    /// `-g <file>:<line>` (VS Code and its forks)
    Goto,
    /// `<file>:<line>` (Sublime Text's `subl`, Zed's `cli`)
    Suffix,
}

/// The command line tool inside an editor's bundle (relative candidates,
/// first existing wins) and its line syntax, for the editors that ship one.
#[cfg(target_os = "macos")]
fn line_tool(bundle_id: &str) -> Option<(&'static [&'static str], LineArgs)> {
    Some(match bundle_id {
        "com.microsoft.VSCode" => (&["Contents/Resources/app/bin/code"], LineArgs::Goto),
        "com.microsoft.VSCodeInsiders" => (
            &[
                "Contents/Resources/app/bin/code-insiders",
                "Contents/Resources/app/bin/code",
            ],
            LineArgs::Goto,
        ),
        "com.visualstudio.code.oss" | "com.vscodium" => {
            (&["Contents/Resources/app/bin/codium"], LineArgs::Goto)
        }
        "com.todesktop.230313mzl4w4u92" => (&["Contents/Resources/app/bin/cursor"], LineArgs::Goto),
        "com.exafunction.windsurf" => (&["Contents/Resources/app/bin/windsurf"], LineArgs::Goto),
        "com.sublimetext.4" | "com.sublimetext.3" | "com.sublimetext.2" => {
            (&["Contents/SharedSupport/bin/subl"], LineArgs::Suffix)
        }
        "dev.zed.Zed" | "dev.zed.Zed-Preview" => (&["Contents/MacOS/cli"], LineArgs::Suffix),
        _ => return None,
    })
}

/// Linux: the found executable is the command line tool itself; its line
/// syntax by editor name (the candidates are relative to the executable,
/// i.e. empty).
#[cfg(not(target_os = "macos"))]
fn line_tool(name: &str) -> Option<(&'static [&'static str], LineArgs)> {
    Some(match name {
        "Visual Studio Code"
        | "Visual Studio Code (Insiders)"
        | "VSCodium"
        | "VSCodium (Insiders)"
        | "Cursor"
        | "Windsurf" => (&[""], LineArgs::Goto),
        "Sublime Text" | "Zed" => (&[""], LineArgs::Suffix),
        _ => return None,
    })
}

/// What [`line_tool`] is keyed by: the bundle identifier on macOS, the
/// editor's name elsewhere.
fn line_tool_key(editor: &FoundEditor) -> &str {
    if cfg!(target_os = "macos") {
        &editor.bundle_id
    } else {
        &editor.name
    }
}

/// The program and arguments that open `target` at `line` (1-based) in
/// `editor`, when its bundle has a command line tool that can.
#[cfg_attr(target_os = "android", allow(dead_code))]
fn line_command(editor: &FoundEditor, target: &Path, line: u32) -> Option<(PathBuf, Vec<String>)> {
    let (candidates, syntax) = line_tool(line_tool_key(editor))?;
    let program = candidates
        .iter()
        .map(|rel| {
            if rel.is_empty() {
                editor.path.clone()
            } else {
                editor.path.join(rel)
            }
        })
        .find(|p| p.is_file())?;
    let at = format!("{}:{line}", target.display());
    let args = match syntax {
        LineArgs::Goto => vec!["-g".to_string(), at],
        LineArgs::Suffix => vec![at],
    };
    Some((program, args))
}

/// Whether `editor` can open a file at a line (see [`launch_at_line`]).
pub fn supports_line(editor: &FoundEditor) -> bool {
    #[cfg(target_os = "android")]
    if editor.bundle_id.starts_with(TERMUX_PREFIX)
        || LINE_APPS
            .iter()
            .any(|app| editor.bundle_id.split('/').next() == Some(app))
    {
        return true;
    }
    line_tool(line_tool_key(editor)).is_some()
}

/// Open `target` at `line` through the editor's command line tool; editors
/// without one (or a missing tool) open the file as [`launch`] does.
pub fn launch_at_line(editor: &FoundEditor, target: &Path, line: u32) -> Result<(), EditorError> {
    #[cfg(target_os = "android")]
    return launch_android(editor, target, Some(line));
    #[cfg(not(target_os = "android"))]
    return match line_command(editor, target, line) {
        Some((program, args)) => {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            apps::spawn_detached(&program, &args).or_else(|_| launch(editor, target))
        }
        None => launch(editor, target),
    };
}

/// GHD `openInExternalEditor` when nothing is installed.
pub fn no_editor_error() -> EditorError {
    EditorError {
        message: format!(
            "No suitable editors installed for Corvene to launch. Install {SUGGESTED_EDITOR_NAME} for your default editor."
        ),
        suggest_default_editor: true,
        open_preferences: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editors() -> Vec<FoundEditor> {
        vec![
            FoundEditor {
                name: "Zed".into(),
                bundle_id: "dev.zed.Zed".into(),
                path: "/Applications/Zed.app".into(),
            },
            FoundEditor {
                name: "Cursor".into(),
                bundle_id: "com.todesktop.230313mzl4w4u92".into(),
                path: "/Applications/Cursor.app".into(),
            },
        ]
    }

    #[test]
    fn termux_commands() {
        let file = Path::new("/storage/emulated/0/repo/src/it's.rs");
        let dir = PathBuf::from("/storage/emulated/0/repo/src");
        assert_eq!(
            termux_command("nvim", file, false, Some(12)),
            (
                "nvim".into(),
                vec!["+12".into(), file.to_string_lossy().into_owned()],
                dir.clone()
            )
        );
        assert_eq!(
            termux_command("hx", file, false, Some(12)).1,
            vec![format!("{}:12", file.display())]
        );
        assert_eq!(
            termux_command("vim", &dir, true, None),
            ("vim".into(), vec![".".into()], dir.clone())
        );
        assert!(termux_command("nano", &dir, true, None).1.is_empty());
        let (program, args, _) = termux_command(TERMUX_DEFAULT_EDITOR, file, false, Some(3));
        assert_eq!(program, "login");
        assert_eq!(
            args,
            vec![
                "-i".to_string(),
                "-c".to_string(),
                "exec sh -c 'exec ${EDITOR:-vi} \"$@\"' sh '+3' \
                 '/storage/emulated/0/repo/src/it'\\''s.rs'"
                    .to_string(),
            ]
        );
    }

    #[test]
    fn code_workspace_only_when_single() {
        let dir = tempfile::tempdir().unwrap();
        let code = FoundEditor {
            name: "Visual Studio Code".into(),
            bundle_id: "com.microsoft.VSCode".into(),
            path: "/Applications/Visual Studio Code.app".into(),
        };
        assert_eq!(code_workspace_file(&code, dir.path()), None);
        std::fs::write(dir.path().join("app.code-workspace"), "{}").unwrap();
        assert_eq!(
            code_workspace_file(&code, dir.path()),
            Some(dir.path().join("app.code-workspace"))
        );
        assert_eq!(code_workspace_file(&editors()[0], dir.path()), None);
        std::fs::write(dir.path().join("other.code-workspace"), "{}").unwrap();
        assert_eq!(code_workspace_file(&code, dir.path()), None);
    }

    #[test]
    fn picks_named_or_first() {
        let e = editors();
        assert_eq!(
            find_editor_or_default(&e, None).unwrap().unwrap().name,
            "Zed"
        );
        assert_eq!(
            find_editor_or_default(&e, Some("Cursor"))
                .unwrap()
                .unwrap()
                .name,
            "Cursor"
        );
        let err = find_editor_or_default(&e, Some("Nope")).unwrap_err();
        assert!(err.open_preferences && err.message.contains("'Nope'"));
        assert!(find_editor_or_default(&[], Some("Zed")).unwrap().is_none());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn line_commands_per_editor() {
        let dir = std::env::temp_dir().join(format!("corvene-editors-{}", std::process::id()));
        let app = dir.join("Code.app");
        let bin = app.join("Contents/Resources/app/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("code"), "").unwrap();
        let code = FoundEditor {
            name: "Visual Studio Code".into(),
            bundle_id: "com.microsoft.VSCode".into(),
            path: app.clone(),
        };
        let (program, args) = line_command(&code, Path::new("/r/src/a.rs"), 12).unwrap();
        assert_eq!(program, bin.join("code"));
        assert_eq!(args, ["-g", "/r/src/a.rs:12"]);
        // the bundle lacks the tool: no line command
        let zed = FoundEditor {
            name: "Zed".into(),
            bundle_id: "dev.zed.Zed".into(),
            path: app,
        };
        assert!(supports_line(&zed));
        assert!(line_command(&zed, Path::new("/r/a"), 1).is_none());
        let bbedit = FoundEditor {
            name: "BBEdit".into(),
            bundle_id: "com.barebones.bbedit".into(),
            path: "/Applications/BBEdit.app".into(),
        };
        assert!(!supports_line(&bbedit));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn linux_line_commands_use_the_executable() {
        let dir = tempfile::tempdir().unwrap();
        let code_bin = dir.path().join("code");
        std::fs::write(&code_bin, "").unwrap();
        let code = FoundEditor {
            name: "Visual Studio Code".into(),
            bundle_id: "/usr/bin/code".into(),
            path: code_bin.clone(),
        };
        let (program, args) = line_command(&code, Path::new("/r/src/a.rs"), 12).unwrap();
        assert_eq!(program, code_bin);
        assert_eq!(args, ["-g", "/r/src/a.rs:12"]);
        let zed = FoundEditor {
            name: "Zed".into(),
            bundle_id: "/usr/bin/zeditor".into(),
            path: code_bin,
        };
        assert_eq!(
            line_command(&zed, Path::new("/r/a"), 3).unwrap().1,
            ["/r/a:3"]
        );
        let kate = FoundEditor {
            name: "Kate".into(),
            bundle_id: "/usr/bin/kate".into(),
            path: "/usr/bin/kate".into(),
        };
        assert!(!supports_line(&kate));
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn linux_paths_resolve_against_home() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(
            resolve_linux_path("/usr/bin/code"),
            PathBuf::from("/usr/bin/code")
        );
        assert_eq!(
            resolve_linux_path("~/.local/bin/zed"),
            home.join(".local/bin/zed")
        );
        assert_eq!(
            resolve_linux_path(".local/share/JetBrains/Toolbox/scripts/idea"),
            home.join(".local/share/JetBrains/Toolbox/scripts/idea")
        );
    }

    #[test]
    fn table_has_no_duplicate_names() {
        let mut names: Vec<&str> = EDITORS
            .iter()
            .chain(EXTRA_EDITORS)
            .map(|(n, _)| *n)
            .collect();
        let before = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(before, names.len());
    }
}
