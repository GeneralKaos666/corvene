package com.wasimaster.corvene.settings

import com.wasimaster.corvene.ffi.gen.DesignStyleVm
import com.wasimaster.corvene.ffi.gen.FlagOptionVm
import com.wasimaster.corvene.ffi.gen.FlagVm
import com.wasimaster.corvene.ffi.gen.FlagsVm
import com.wasimaster.corvene.ffi.gen.PresetVm
import com.wasimaster.corvene.ffi.gen.RepositorySettingsVm
import com.wasimaster.corvene.ffi.gen.SettingsVm
import com.wasimaster.corvene.ffi.gen.ThemeVm

/** The engine's defaults for a fresh store (every confirmation on). */
val SampleSettings = SettingsVm(
    designStyle = DesignStyleVm.GIT_HUB_MOBILE,
    designStyleSetting = DesignStyleVm.GIT_HUB_MOBILE,
    designStylePinned = false,
    theme = ThemeVm.SYSTEM,
    welcomeCompleted = true,
    confirmDiscardChanges = true,
    confirmForcePush = true,
    confirmRepositoryRemoval = true,
    notificationsEnabled = true,
    repositoryIndicatorsEnabled = true,
    hideWhitespaceInChangesDiff = false,
    hideWhitespaceInHistoryDiff = false,
    showDiffCheckMarks = true,
    underlineLinks = true,
    confirmCheckoutCommit = true,
    confirmUndoCommit = true,
    confirmDiscardStash = true,
    confirmCommitFilteredChanges = true,
    showCommitLengthWarning = false,
    commitSpellcheckEnabled = false,
    historyFirstParent = false,
    uncommittedChangesStrategy = "ask",
    externalEditor = null,
    shell = null,
    cloneDir = null,
)

fun flag(
    slug: String,
    id: Int,
    title: String,
    category: String = "Changes",
    kind: String = "toggle",
    value: String = "false",
    isOn: Boolean = value == "true",
    nature: String = "Feature",
    overridden: Boolean = false,
    restart: Boolean = false,
    restartPending: Boolean = false,
    available: Boolean = true,
    options: List<FlagOptionVm> = emptyList(),
    min: Long? = null,
    max: Long? = null,
    unit: String? = null,
    summary: String = "What $title changes.",
) = FlagVm(
    ident = "$id-$slug",
    slug = slug,
    id = id.toUShort(),
    title = title,
    summary = summary,
    ghdBehaviour = "GitHub Desktop does not.",
    category = category,
    nature = nature,
    kind = kind,
    options = options,
    min = min,
    max = max,
    unit = unit,
    value = value,
    valueLabel = options.firstOrNull { it.value == value }?.label ?: value,
    isOn = isOn,
    overridden = overridden,
    presetValue = value,
    ghdValue = "false",
    restart = restart,
    restartPending = restartPending,
    available = available,
)

val SampleFlags = FlagsVm(
    preset = "corvene",
    presets = listOf(
        PresetVm("github-desktop", "GitHub Desktop", "Every flag as GitHub Desktop 3.6.6 behaves."),
        PresetVm("familiar", "Familiar", "GitHub Desktop with its bugs fixed."),
        PresetVm("corvene", "Corvene", "Corvene's defaults: the fixes and the features most people want."),
        PresetVm("max", "Max", "Every feature on."),
    ),
    categories = listOf("Changes", "History", "Appearance"),
    flags = listOf(
        flag("renamed-files-filter", 705, "Renamed files filter", value = "true", overridden = true),
        flag("diff-check-marks-fix", 851, "Check marks on wrapped lines", nature = "Bug fix", value = "true"),
        flag(
            "history-page-size", 402, "History page size", category = "History", kind = "number", value = "100",
            isOn = true, min = 50, max = 120, unit = "commits",
        ),
        flag(
            "design-style", 112, "Design style", category = "Appearance", kind = "select", value = "auto",
            options = listOf(FlagOptionVm("auto", "Setting"), FlagOptionVm("github-desktop", "GitHub Desktop"), FlagOptionVm("material", "Material")),
            restart = true,
        ),
        flag("commit-template", 201, "Commit template", category = "Changes", kind = "text", value = ""),
        flag("tree-sitter-highlighting", 105, "Tree-sitter highlighting", category = "Appearance", available = false),
    ),
)

val SampleRepositorySettings = RepositorySettingsVm(
    repo = 7u,
    remoteName = "origin",
    remoteUrl = "https://github.com/wasi-master/corvene.git",
    gitignore = "target/\n*.log\n",
    localName = null,
    localEmail = null,
    globalName = "Wasi Master",
    globalEmail = "wasi@example.com",
    autocrlf = false,
    localAutocrlf = null,
)
