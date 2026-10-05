/**
 * What the "Help me pick" guide asks and says: the architectures, packages
 * and editions of each operating system, how to tell which one a machine
 * needs, and which release file the answers lead to. The facts follow the
 * README's Install section and site/src/pages/docs/installation.astro.
 */

import { assetNames, type AndroidAbi, type Edition } from "../lib/assets";
import { BREW_COMMAND } from "../lib/github";

export type OsId = "macos" | "windows" | "linux" | "android";
export type ArchId = "universal" | "arm64" | "x86_64" | "x86" | "armv7";
export type FormatId =
  | "dmg"
  | "mac-zip"
  | "brew"
  | "setup"
  | "msi"
  | "portable"
  | "deb"
  | "rpm"
  | "appimage"
  | "flatpak"
  | "snap"
  | "tarball"
  | "compose"
  | "legacy";

export interface OsOption {
  id: OsId;
  title: string;
  blurb: string;
  requires: string;
}

export interface ArchOption {
  id: ArchId;
  title: string;
  /** the title in the answer chips, when it differs */
  short?: string;
  /** what the same thing is called elsewhere (file names, uname, vendors) */
  aka: string[];
  hint: string;
  /** chips or machines that have it */
  examples: string[];
  badge?: string;
  /** a rarely needed one, drawn quieter */
  minor?: boolean;
}

export interface FormatOption {
  id: FormatId;
  title: string;
  /** the file type or package name, shown as a tag */
  tag: string;
  summary: string;
  /** distributions or situations it is for */
  forWhom?: string;
  good: string[];
  mind: string[];
  badge?: string;
  /** architectures it is built for; every one when absent */
  archs?: ArchId[];
  /** no edition choice: Homebrew installs the Standard edition */
  standardOnly?: boolean;
}

export interface CheckStep {
  how: string;
  /** a command to run, when there is one */
  command?: string;
  /** what you might see → the answer to pick */
  readings: { see: string; pick: ArchId }[];
}

export const OS_OPTIONS: OsOption[] = [
  { id: "macos", title: "macOS", blurb: "MacBook, iMac, Mac mini, Mac Studio", requires: "macOS 11+ on Apple Silicon, 10.15.7+ on Intel" },
  { id: "windows", title: "Windows", blurb: "Laptops and desktops, x64 or ARM", requires: "Windows 10 or 11" },
  { id: "linux", title: "Linux", blurb: "Ubuntu, Fedora, Arch, Debian and the rest", requires: "glibc 2.35+, X11 or Wayland, Vulkan" },
  { id: "android", title: "Android", blurb: "Phones, tablets and Chromebooks", requires: "Android 8.0 or newer" },
];

export const ARCH_QUESTION: Record<OsId, { title: string; lead: string }> = {
  macos: { title: "Which chip is in your Mac?", lead: "Apple moved from Intel to its own chips in late 2020. Not sure? Universal runs on both." },
  windows: { title: "Which processor does your PC have?", lead: "Almost every PC is x64. Only a few newer laptops run Windows on ARM." },
  linux: { title: "Which processor architecture?", lead: "Desktops and laptops are nearly always x86_64. Boards like the Raspberry Pi are ARM." },
  android: { title: "Which processor does your device have?", lead: "Nearly every phone from the last several years is arm64. Universal fits anything." },
};

export const ARCH_OPTIONS: Record<OsId, ArchOption[]> = {
  macos: [
    {
      id: "arm64",
      title: "Apple Silicon",
      aka: ["arm64", "aarch64", "M-series"],
      hint: "Every Mac with an M chip: MacBook Air and Pro from late 2020, iMac from 2021, Mac mini from 2020, Mac Studio and the 2023 Mac Pro.",
      examples: ["M1", "M2", "M3", "M4", "M5", "Pro, Max, Ultra"],
    },
    {
      id: "x86_64",
      title: "Intel",
      aka: ["x86_64", "x64", "amd64", "Intel 64"],
      hint: "Macs from 2006 to 2020 with an Intel processor. Corvene needs macOS 10.15.7 Catalina or newer on them.",
      examples: ["Core i3", "Core i5", "Core i7", "Core i9", "Xeon W"],
    },
    {
      id: "universal",
      title: "Not sure, or both",
      short: "Universal",
      aka: ["Universal 2", "fat binary"],
      hint: "One app with both architectures inside: native on every Mac. A larger download, handy on a drive shared between Macs. Homebrew and the updater use it.",
      examples: ["Any Mac"],
      badge: "Always works",
    },
  ],
  windows: [
    {
      id: "x86_64",
      title: "x64",
      aka: ["x86_64", "amd64", "x86-64", "Intel 64", "64-bit"],
      hint: "Nearly every Windows PC: anything with an Intel or AMD processor running 64-bit Windows.",
      examples: ["Intel Core i3 to i9", "Core Ultra", "AMD Ryzen", "Athlon", "Pentium, Celeron"],
      badge: "Most PCs",
    },
    {
      id: "arm64",
      title: "ARM64",
      aka: ["aarch64", "Windows on ARM", "ARMv8"],
      hint: "Copilot+ and other ARM laptops, and Windows in Parallels or UTM on an Apple Silicon Mac. The x64 build also runs there, emulated and slower.",
      examples: ["Snapdragon X Elite", "Snapdragon X Plus", "Snapdragon 8cx", "Microsoft SQ1 to SQ3", "Surface Pro X"],
    },
    {
      id: "x86",
      title: "32-bit",
      aka: ["x86", "i686", "i386", "IA-32", "Win32"],
      hint: "Only for 32-bit Windows 10, usually on old or very low-end machines. Windows 11 is 64-bit only. Built but less tested.",
      examples: ["Atom netbooks", "Early Windows tablets"],
      minor: true,
    },
  ],
  linux: [
    {
      id: "x86_64",
      title: "x86_64",
      aka: ["amd64", "x64", "x86-64", "Intel 64"],
      hint: "Desktops, laptops and most servers with an Intel or AMD processor.",
      examples: ["Intel Core", "AMD Ryzen", "Xeon", "EPYC"],
      badge: "Most machines",
    },
    {
      id: "arm64",
      title: "ARM64",
      aka: ["aarch64", "ARMv8"],
      hint: "64-bit ARM: single-board computers on a 64-bit OS, ARM laptops, cloud servers, and Linux VMs on an Apple Silicon Mac.",
      examples: ["Raspberry Pi 4, 5, 400", "Asahi Linux", "Snapdragon X", "AWS Graviton", "Ampere"],
    },
    {
      id: "x86",
      title: "32-bit x86",
      aka: ["i686", "i386", "x86", "IA-32"],
      hint: "32-bit distributions on old PCs. Built but little tested.",
      examples: ["Pentium 4", "Atom netbooks"],
      minor: true,
    },
    {
      id: "armv7",
      title: "32-bit ARM",
      aka: ["armhf", "armv7l", "armv7hl", "ARM32"],
      hint: "32-bit ARM boards and 32-bit Raspberry Pi OS. Built but little tested.",
      examples: ["Raspberry Pi 2, 3", "BeagleBone"],
      minor: true,
    },
  ],
  android: [
    {
      id: "arm64",
      title: "arm64",
      aka: ["arm64-v8a", "aarch64", "ARMv8"],
      hint: "Practically every phone and tablet of the last several years, and ARM Chromebooks.",
      examples: ["Snapdragon", "MediaTek Dimensity", "Helio", "Exynos", "Google Tensor"],
      badge: "Most devices",
    },
    {
      id: "universal",
      title: "Not sure: universal",
      short: "Universal",
      aka: ["all ABIs", "fat APK"],
      hint: "All four architectures in one APK. Installs on anything, at several times the size. The Full edition has no universal APK.",
      examples: ["Any device"],
      badge: "Always works",
    },
    {
      id: "armv7",
      title: "armv7 (32-bit)",
      aka: ["armeabi-v7a", "ARM32", "armhf"],
      hint: "Older or budget phones, and Android Go devices that run 32-bit Android even on a 64-bit chip.",
      examples: ["Android Go", "Older budget phones"],
    },
    {
      id: "x86_64",
      title: "x86_64",
      aka: ["x86-64", "amd64"],
      hint: "Chromebooks with Intel or AMD processors, the Android Emulator on a PC, and Android-x86 installs.",
      examples: ["Intel Chromebooks", "AMD Chromebooks", "Android Emulator"],
      minor: true,
    },
    {
      id: "x86",
      title: "x86 (32-bit)",
      aka: ["i686", "x86"],
      hint: "32-bit emulator images and old Intel Atom phones and tablets.",
      examples: ["Old emulator images", "Atom tablets"],
      minor: true,
    },
  ],
};

export const ARCH_CHECK: Record<OsId, CheckStep[]> = {
  macos: [
    {
      how: "Apple menu › About This Mac.",
      readings: [
        { see: "Chip: Apple M1, M2, M3…", pick: "arm64" },
        { see: "Processor: … Intel Core …", pick: "x86_64" },
      ],
    },
    {
      how: "Or in Terminal:",
      command: "uname -m",
      readings: [
        { see: "arm64", pick: "arm64" },
        { see: "x86_64", pick: "x86_64" },
      ],
    },
  ],
  windows: [
    {
      how: "Settings › System › About, then look at System type.",
      readings: [
        { see: "64-bit operating system, x64-based processor", pick: "x86_64" },
        { see: "64-bit operating system, ARM-based processor", pick: "arm64" },
        { see: "32-bit operating system", pick: "x86" },
      ],
    },
    {
      how: "Or in Command Prompt:",
      command: "echo %PROCESSOR_ARCHITECTURE%",
      readings: [
        { see: "AMD64", pick: "x86_64" },
        { see: "ARM64", pick: "arm64" },
        { see: "x86", pick: "x86" },
      ],
    },
  ],
  linux: [
    {
      how: "In a terminal:",
      command: "uname -m",
      readings: [
        { see: "x86_64", pick: "x86_64" },
        { see: "aarch64", pick: "arm64" },
        { see: "i686, i386", pick: "x86" },
        { see: "armv7l", pick: "armv7" },
      ],
    },
    {
      how: "On Debian and Ubuntu, dpkg names it like the .deb files:",
      command: "dpkg --print-architecture",
      readings: [
        { see: "amd64", pick: "x86_64" },
        { see: "arm64", pick: "arm64" },
        { see: "i386", pick: "x86" },
        { see: "armhf", pick: "armv7" },
      ],
    },
  ],
  android: [
    {
      how: "Settings › About phone names the processor or model; look the chip up if it is not listed above. A computer with adb can ask directly:",
      command: "adb shell getprop ro.product.cpu.abi",
      readings: [
        { see: "arm64-v8a", pick: "arm64" },
        { see: "armeabi-v7a", pick: "armv7" },
        { see: "x86_64", pick: "x86_64" },
        { see: "x86", pick: "x86" },
      ],
    },
  ],
};

export const FORMAT_QUESTION: Record<OsId, { title: string; lead: string }> = {
  macos: { title: "How would you like to install it?", lead: "All three install the same app." },
  windows: { title: "Which kind of package?", lead: "The same app in three wrappers: they differ in where it goes and how it updates." },
  linux: { title: "Which package fits your system?", lead: "Pick what your distribution uses, or a format that runs anywhere." },
  android: {
    title: "Which Android app?",
    lead: "Two apps share Corvene's engine. They install side by side, so you can try both.",
  },
};

const DESKTOP_ARCHS: ArchId[] = ["x86_64", "arm64"];

export const FORMAT_OPTIONS: Record<OsId, FormatOption[]> = {
  macos: [
    {
      id: "dmg",
      title: "Disk image",
      tag: ".dmg",
      summary: "Open it and drag Corvene into Applications.",
      good: ["The usual Mac way", "Updates itself from inside the app"],
      mind: ["macOS asks once before the first launch: the app is not notarized"],
      badge: "Most people",
    },
    {
      id: "mac-zip",
      title: "Zip archive",
      tag: ".zip",
      summary: "The same app, zipped. It is what the built-in updater downloads.",
      good: ["Easy to script: unzip, move, done", "Suits MDM and provisioning tools"],
      mind: ["The same one-time prompt before the first launch"],
    },
    {
      id: "brew",
      title: "Homebrew",
      tag: "brew",
      summary: "One command, if you already use Homebrew.",
      good: ["No Gatekeeper prompt: the cask clears it", "brew upgrade updates it, and so does the app", "Adds the corvene command line tool"],
      mind: ["Installs the universal Standard edition"],
      badge: "Recommended",
      standardOnly: true,
    },
  ],
  windows: [
    {
      id: "setup",
      title: "Installer",
      tag: ".exe",
      summary: "Installs for your user only, without administrator rights.",
      good: ["Updates itself", "Start menu entry and corvene on your PATH", "\"Open in Corvene\" in Explorer's folder menu"],
      mind: ["SmartScreen warns on first run: More info, then Run anyway"],
      badge: "Recommended",
    },
    {
      id: "msi",
      title: "Windows Installer",
      tag: ".msi",
      summary: "A machine-wide install into Program Files, made for IT.",
      good: ["Every user on the PC", "Silent installs with msiexec /qn", "Group Policy and Intune deployment"],
      mind: ["Needs administrator rights", "Does not update itself: the app only says a release is out"],
    },
    {
      id: "portable",
      title: "Portable",
      tag: ".zip",
      summary: "Unzip anywhere and run. A USB stick works.",
      good: ["No install and no administrator rights", "Carry it between PCs"],
      mind: ["No self-update, no PATH entry, no Start menu entry"],
    },
  ],
  linux: [
    {
      id: "deb",
      title: "Debian package",
      tag: ".deb",
      summary: "Installed and removed with apt like any other package.",
      forWhom: "Debian, Ubuntu, Linux Mint, Pop!_OS, elementary OS, Zorin",
      good: ["apt pulls in the dependencies", "corvene command in /usr/bin"],
      mind: ["Install the next .deb to update"],
      badge: "Ubuntu and friends",
    },
    {
      id: "rpm",
      title: "RPM package",
      tag: ".rpm",
      summary: "Installed with dnf or zypper.",
      forWhom: "Fedora, RHEL, Rocky, AlmaLinux, openSUSE",
      good: ["Dependencies handled by dnf or zypper", "corvene command in /usr/bin"],
      mind: ["Install the next .rpm to update"],
    },
    {
      id: "appimage",
      title: "AppImage",
      tag: ".AppImage",
      summary: "One file that runs on any distribution.",
      forWhom: "Any distribution",
      good: ["No root needed", "Updates itself in place", "Adds itself to the app menu on first launch"],
      mind: ["Your package manager does not know about it"],
    },
    {
      id: "flatpak",
      title: "Flatpak",
      tag: ".flatpak",
      summary: "Sandboxed, with its own git. The runtime comes from Flathub.",
      forWhom: "Fedora Silverblue, Bazzite, SteamOS, any Flatpak setup",
      good: ["Sandboxed and self-contained", "Shares its runtime with other Flatpaks"],
      mind: ["Sees your home folder unless you grant more", "No command line tool"],
      archs: DESKTOP_ARCHS,
    },
    {
      id: "snap",
      title: "Snap",
      tag: ".snap",
      summary: "Strictly confined, with git, Git LFS and OpenSSH inside.",
      forWhom: "Ubuntu and other snap systems",
      good: ["Fully self-contained", "Strict confinement"],
      mind: ["Installed with --dangerous, not from the Snap Store", "Two snap connect commands for accounts and SSH"],
      archs: DESKTOP_ARCHS,
    },
    {
      id: "tarball",
      title: "Tarball",
      tag: ".tar.gz",
      summary: "The .deb's files in a plain archive: run it in place or unpack it into /.",
      forWhom: "Arch (what the AUR's corvene-bin installs), Gentoo, Void, anything else",
      good: ["Works without any package manager"],
      mind: ["No updates and no menu entry by itself"],
    },
    {
      id: "brew",
      title: "Homebrew",
      tag: "brew",
      summary: "Installs the AppImage as ~/Applications/Corvene.AppImage.",
      forWhom: "Homebrew on Linux users",
      good: ["brew upgrade updates it", "Cleans up after itself with brew uninstall --zap"],
      mind: ["Standard edition only"],
      archs: DESKTOP_ARCHS,
      standardOnly: true,
    },
  ],
  android: [
    {
      id: "compose",
      title: "Corvene",
      tag: "New",
      summary: "Rebuilt for phones with Jetpack Compose, over the same engine as the desktop app.",
      forWhom: "Phones, held in one hand",
      good: ["Touch-first screens made for portrait", "Changes, history, branches, conflicts and cloning", "Android's back gesture, share sheet and Termux"],
      mind: ["New and in progress: some desktop features are still coming"],
      badge: "For phones",
    },
    {
      id: "legacy",
      title: "Corvene Legacy",
      tag: "Desktop",
      summary: "The desktop app itself in a native window: GitHub Desktop 1:1, every menu, dialog and shortcut.",
      forWhom: "Tablets, Chromebooks, Samsung DeX, desktop mode",
      good: ["Everything the desktop app has", "At home with a keyboard and mouse on a big screen"],
      mind: ["Desktop-sized controls are small on a phone"],
    },
  ],
};

export const EDITION_FACTS = {
  /** documented in installation.astro */
  standardInstalled: "about 23 MB",
  fullExtraPerArch: 170,
  /** the grammars compressed, per architecture: v0.1.0's universal .dmg grew by 61 MB */
  fullDownloadExtraPerArch: 30,
};

/**
 * Download sizes in MB measured on the v0.1.0 release, for when the live
 * release does not list a file yet. Keyed by package and architecture.
 */
const MEASURED_MB: Record<string, { standard: number; full?: number }> = {
  "dmg:universal": { standard: 34, full: 95 },
  "mac-zip:universal": { standard: 31, full: 81 },
  "deb:x86_64": { standard: 16 },
  "deb:arm64": { standard: 15 },
  "appimage:x86_64": { standard: 21 },
  "appimage:arm64": { standard: 20 },
};

/** An approximate download size in MB, or null when nothing was measured. */
export function estimateMB(edition: Edition, os: OsId, arch: ArchId, format: FormatId): number | null {
  const measured = MEASURED_MB[`${format}:${arch}`];
  if (!measured) return null;
  if (edition === "standard") return measured.standard;
  if (measured.full) return measured.full;
  // Android stores native code uncompressed: no estimate there
  if (os === "android") return null;
  const archs = os === "macos" && arch === "universal" ? 2 : 1;
  return measured.standard + EDITION_FACTS.fullDownloadExtraPerArch * archs;
}

/** The release file the answers lead to, or null when it is not built. */
export function resolveFile(version: string, edition: Edition, os: OsId, arch: ArchId, format: FormatId): string | null {
  const names = assetNames(version, edition);
  const win = { x86_64: "x86_64", arm64: "aarch64", x86: "i686" } as const;
  switch (format) {
    case "dmg":
    case "mac-zip": {
      if (arch !== "universal" && arch !== "arm64" && arch !== "x86_64") return null;
      return format === "dmg" ? names.macos.dmg(arch) : names.macos.zip(arch);
    }
    case "setup":
    case "msi":
    case "portable": {
      if (arch !== "x86_64" && arch !== "arm64" && arch !== "x86") return null;
      return names.windows[format](win[arch]);
    }
    case "deb": {
      const deb = { x86_64: "amd64", arm64: "arm64", x86: "i386", armv7: "armhf" } as const;
      return arch === "universal" ? null : names.linux.deb(deb[arch]);
    }
    case "rpm": {
      const rpm = { x86_64: "x86_64", arm64: "aarch64", x86: "i686", armv7: "armv7hl" } as const;
      return arch === "universal" ? null : names.linux.rpm(rpm[arch]);
    }
    case "appimage":
    case "tarball": {
      const linux = { x86_64: "x86_64", arm64: "aarch64", x86: "i686", armv7: "armhf" } as const;
      if (arch === "universal") return null;
      return format === "appimage" ? names.linux.appimage(linux[arch]) : names.linux.tarball(linux[arch]);
    }
    case "flatpak":
    case "snap": {
      if (arch !== "x86_64" && arch !== "arm64") return null;
      const a = arch === "arm64" ? "aarch64" : "x86_64";
      return format === "flatpak" ? names.linux.flatpak(a) : names.linux.snap(a);
    }
    case "compose":
    case "legacy": {
      // the Full edition has no universal APK
      if (edition === "full" && arch === "universal") return null;
      return names.android.apk(arch as AndroidAbi, format);
    }
    case "brew":
      return null;
  }
  void os;
  return null;
}

/** The architecture as the files spell it, for the summary line. */
export function archLabel(os: OsId, arch: ArchId): string {
  const option = ARCH_OPTIONS[os].find((a) => a.id === arch);
  return option?.short ?? option?.title ?? arch;
}

export interface InstallStep {
  text: string;
  command?: string;
}

/** How to install what was picked, step by step. */
export function installSteps(os: OsId, format: FormatId, file: string | null): InstallStep[] {
  const f = file ?? "";
  switch (format) {
    case "brew":
      return [
        { text: "Run this in a terminal:", command: BREW_COMMAND },
        { text: os === "macos" ? "Open Corvene from Applications. No security prompt." : "Start it from your app menu, or run ~/Applications/Corvene.AppImage." },
      ];
    case "dmg":
      return [
        { text: "Open the .dmg and drag Corvene into Applications." },
        { text: "Open it. macOS says it cannot verify the app: click Done, then System Settings › Privacy & Security › Open Anyway." },
        { text: "Or clear the flag once from Terminal:", command: "xattr -d com.apple.quarantine /Applications/Corvene.app" },
      ];
    case "mac-zip":
      return [
        { text: "Double-click the .zip, then move Corvene.app into Applications." },
        { text: "Open it, and allow it once in System Settings › Privacy & Security › Open Anyway." },
        { text: "Or clear the flag from Terminal:", command: "xattr -d com.apple.quarantine /Applications/Corvene.app" },
      ];
    case "setup":
      return [
        { text: "Run the installer. No administrator rights needed." },
        { text: "If SmartScreen appears, click More info, then Run anyway. The packages are not code-signed yet." },
        { text: "Corvene lands in the Start menu, and corvene works in any terminal." },
      ];
    case "msi":
      return [
        { text: "Double-click it, or install silently for every user from an administrator prompt:", command: `msiexec /i ${f} /qn` },
        { text: "Deploy a newer .msi the same way to update." },
      ];
    case "portable":
      return [
        { text: "Unzip it anywhere: a folder, another drive, a USB stick." },
        { text: "Run Corvene\\corvene.exe. If SmartScreen appears, click More info, then Run anyway." },
      ];
    case "deb":
      return [{ text: "Install it with apt:", command: `sudo apt install ./${f}` }, { text: "Start Corvene from your app menu, or run corvene." }];
    case "rpm":
      return [
        { text: "Install it with dnf (zypper install on openSUSE):", command: `sudo dnf install ./${f}` },
        { text: "Start Corvene from your app menu, or run corvene." },
      ];
    case "appimage":
      return [
        { text: "Make it executable and run it:", command: `chmod +x ${f} && ./${f}` },
        { text: "On first launch it adds itself to the app menu, and from then on it updates itself." },
      ];
    case "flatpak":
      return [
        { text: "Install it for your user:", command: `flatpak install --user ${f}` },
        { text: "To let it see folders outside your home:", command: "flatpak override --user --filesystem=<path> com.wasimaster.corvene" },
      ];
    case "snap":
      return [
        { text: "Install it, then connect accounts and SSH:", command: `sudo snap install --dangerous ${f}\nsudo snap connect corvene:password-manager-service\nsudo snap connect corvene:ssh-keys` },
      ];
    case "tarball":
      return [
        { text: "Unpack it and run it in place (or unpack into / as root):", command: `tar xzf ${f} && ./usr/lib/corvene/corvene` },
        { text: "On Arch, the AUR's corvene-bin and corvene-full-bin install this same archive." },
      ];
    case "compose":
    case "legacy":
      return [
        { text: "Download it on the device itself, or copy it over." },
        { text: "Open the APK. Android asks once to let your browser or file manager install apps." },
        { text: "Install newer APKs the same way: repositories, accounts and settings stay." },
      ];
  }
  return [];
}
