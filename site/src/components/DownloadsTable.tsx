import { useEffect, useState, type ReactNode } from "react";
import MacQuarantineModal from "./MacQuarantineModal";
import CopyButton from "./CopyButton";
import { AndroidIcon, AppleIcon, LinuxIcon, WindowsIcon } from "./PlatformIcons";
import { useAssetPopover } from "./AssetPopover";
import { assetNames, type Edition } from "../lib/assets";
import { BREW_COMMAND, RELEASES_URL, assetUrl, fetchLatestRelease, type LatestRelease } from "../lib/github";

interface Props {
  fallbackVersion: string;
}

interface DownloadLink {
  label: string;
  file: string;
  /** a less common architecture, drawn dimmer */
  minor?: boolean;
}

interface DownloadGroup {
  title: string;
  note?: string;
  links: DownloadLink[];
}

interface PlatformSpec {
  id: string;
  icon: ReactNode;
  title: string;
  subtitle: string;
  groups: DownloadGroup[];
  footer: ReactNode;
  /** macOS downloads open the Gatekeeper note */
  mac?: boolean;
}

export default function DownloadsTable({ fallbackVersion }: Props) {
  const [release, setRelease] = useState<LatestRelease | null>(null);
  const [edition, setEdition] = useState<Edition>("standard");
  const [modalUrl, setModalUrl] = useState<string | null>(null);

  useEffect(() => {
    fetchLatestRelease().then(setRelease);
  }, []);

  const version = release?.version ?? fallbackVersion;
  const assets = release?.assets ?? [];
  const url = (file: string) => assetUrl(version, file, assets);
  const names = assetNames(version, edition);
  const full = edition === "full";
  const { handlers, popover } = useAssetPopover(assets, version, release !== null);

  const platforms: PlatformSpec[] = [
    {
      id: "macos",
      mac: true,
      icon: <AppleIcon />,
      title: "macOS",
      subtitle: "macOS 11 or newer on Apple Silicon, 10.15.7 or newer on Intel.",
      groups: [
        {
          title: "Disk image (.dmg)",
          links: [
            { label: "Universal", file: names.macos.dmg("universal") },
            { label: "Apple Silicon", file: names.macos.dmg("arm64") },
            { label: "Intel", file: names.macos.dmg("x86_64") },
          ],
        },
        {
          title: "Zip (what the app's updater installs)",
          links: [
            { label: "Universal", file: names.macos.zip("universal") },
            { label: "Apple Silicon", file: names.macos.zip("arm64") },
            { label: "Intel", file: names.macos.zip("x86_64") },
          ],
        },
      ],
      footer: (
        <div className="space-y-2">
          <div className="flex items-center justify-between gap-2 text-[11px] font-semibold text-fg">
            <span>Homebrew (recommended: no Gatekeeper prompt)</span>
          </div>
          <div className="flex items-center justify-between gap-2 rounded-lg border border-edge bg-canvas-inset py-1.5 pl-3 pr-1.5 font-mono text-xs text-accent-muted">
            <code className="truncate">
              <span className="select-none text-fg-subtle">$ </span>
              {BREW_COMMAND}
            </code>
            <CopyButton text={BREW_COMMAND} />
          </div>
        </div>
      ),
    },
    {
      id: "windows",
      icon: <WindowsIcon />,
      title: "Windows",
      subtitle: "Windows 10 and 11 on x64, ARM64 and 32-bit x86.",
      groups: [
        {
          title: "Installer (per user, updates itself)",
          links: [
            { label: "x64", file: names.windows.setup("x86_64") },
            { label: "ARM64", file: names.windows.setup("aarch64") },
            { label: "32-bit", file: names.windows.setup("i686"), minor: true },
          ],
        },
        {
          title: "Windows Installer (.msi, machine-wide)",
          note: "Group Policy, Intune, silent installs",
          links: [
            { label: "x64", file: names.windows.msi("x86_64") },
            { label: "ARM64", file: names.windows.msi("aarch64") },
            { label: "32-bit", file: names.windows.msi("i686"), minor: true },
          ],
        },
        {
          title: "Portable (.zip)",
          note: "Unzip and run, no install",
          links: [
            { label: "x64", file: names.windows.portable("x86_64") },
            { label: "ARM64", file: names.windows.portable("aarch64") },
            { label: "32-bit", file: names.windows.portable("i686"), minor: true },
          ],
        },
      ],
      footer: <p>Windows builds are new and have had less testing than macOS and Linux.</p>,
    },
    {
      id: "linux",
      icon: <LinuxIcon />,
      title: "Linux",
      subtitle: "glibc 2.35 or newer, X11 or Wayland, a Vulkan driver.",
      groups: [
        {
          title: "Debian, Ubuntu, Mint (.deb)",
          links: [
            { label: "amd64", file: names.linux.deb("amd64") },
            { label: "arm64", file: names.linux.deb("arm64") },
            { label: "i386", file: names.linux.deb("i386"), minor: true },
            { label: "armhf", file: names.linux.deb("armhf"), minor: true },
          ],
        },
        {
          title: "Fedora, openSUSE, RHEL (.rpm)",
          links: [
            { label: "x86_64", file: names.linux.rpm("x86_64") },
            { label: "aarch64", file: names.linux.rpm("aarch64") },
            { label: "i686", file: names.linux.rpm("i686"), minor: true },
            { label: "armv7hl", file: names.linux.rpm("armv7hl"), minor: true },
          ],
        },
        {
          title: "AppImage",
          note: "Any distribution, updates itself",
          links: [
            { label: "x86_64", file: names.linux.appimage("x86_64") },
            { label: "aarch64", file: names.linux.appimage("aarch64") },
            { label: "i686", file: names.linux.appimage("i686"), minor: true },
            { label: "armhf", file: names.linux.appimage("armhf"), minor: true },
          ],
        },
        {
          title: "Flatpak and snap",
          note: "Sandboxed, with their own git",
          links: [
            { label: "x86_64 .flatpak", file: names.linux.flatpak("x86_64") },
            { label: "aarch64 .flatpak", file: names.linux.flatpak("aarch64") },
            { label: "x86_64 .snap", file: names.linux.snap("x86_64") },
            { label: "aarch64 .snap", file: names.linux.snap("aarch64") },
          ],
        },
        {
          title: "Tarball (.tar.gz)",
          note: "Any distribution; what the AUR packages install",
          links: [
            { label: "x86_64", file: names.linux.tarball("x86_64") },
            { label: "aarch64", file: names.linux.tarball("aarch64") },
            { label: "i686", file: names.linux.tarball("i686"), minor: true },
            { label: "armhf", file: names.linux.tarball("armhf"), minor: true },
          ],
        },
      ],
      footer: (
        <p>
          Homebrew installs the AppImage too: <code className="font-mono text-fg-secondary">{BREW_COMMAND}</code>. The 32-bit
          and ARMv7 packages are built but little tested.
        </p>
      ),
    },
    {
      id: "android",
      icon: <AndroidIcon />,
      title: "Android",
      subtitle: "Android 8.0 or newer, experimental. git, OpenSSH and Git LFS are bundled.",
      groups: (["compose", "legacy"] as const).map((app) => ({
        title: app === "compose" ? "Corvene (new, built for phones)" : "Corvene Legacy (the desktop app)",
        note: app === "compose" ? "Phones, in progress" : "Tablets, Chromebooks, DeX",
        links: [
          { label: "arm64 (most devices)", file: names.android.apk("arm64", app) },
          ...(full ? [] : [{ label: "Universal (every ABI)", file: names.android.apk("universal", app) }]),
          { label: "armv7 (32-bit phones)", file: names.android.apk("armv7", app) },
          { label: "x86_64 (Chromebooks)", file: names.android.apk("x86_64", app), minor: true },
          { label: "x86", file: names.android.apk("x86", app), minor: true },
        ],
      })),
      footer: (
        <p>
          The two apps install side by side. Open the APK on the device to install; installing a newer one keeps
          repositories, accounts and settings.
        </p>
      ),
    },
  ];

  return (
    <div className="mx-auto w-full max-w-6xl space-y-8 rounded-2xl border border-edge bg-canvas p-6 shadow-2xl sm:p-8" {...handlers}>
      {popover}

      <div className="flex flex-col gap-4 border-b border-edge-muted pb-6 md:flex-row md:items-end md:justify-between">
        <div>
          <h3 className="text-xl font-bold text-fg">Every package</h3>
          <p className="mt-1 text-sm text-fg-muted">
            Release <span className="font-mono text-accent">v{version}</span>
            {release ? "" : " (checking GitHub for the latest…)"}. Hover a link for its size and sha256.
          </p>
        </div>

        <div className="flex flex-col items-start gap-1.5 md:items-end">
          <div role="radiogroup" aria-label="Edition" className="flex rounded-xl border border-edge bg-canvas-subtle p-1">
          <EditionButton selected={!full} onSelect={() => setEdition("standard")} title="Standard" note="~23 MB" colorClass="bg-success-emphasis" />
          <EditionButton selected={full} onSelect={() => setEdition("full")} title="Full" note="+170 MB" colorClass="bg-done-emphasis" />
          </div>
          <p className="text-[11px] text-fg-subtle">{full ? "Every tree-sitter grammar built in, for offline machines." : "Syntax grammars for 300+ languages download on demand."}</p>
        </div>
      </div>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
        {platforms.map((p) => (
          <PlatformCard key={p.id} spec={p} full={full} getUrl={url} onDownload={(href) => p.mac && setModalUrl(href)} />
        ))}
      </div>

      <p className="text-xs text-fg-subtle">
        The Full edition replaces the Standard one and updates to the next Full release. Checksums for every file are on the{" "}
        <a href={release?.htmlUrl ?? RELEASES_URL} className="text-fg-muted hover:text-fg hover:underline" target="_blank" rel="noopener noreferrer">
          release page
        </a>
        .
      </p>

      <MacQuarantineModal open={modalUrl !== null} onClose={() => setModalUrl(null)} downloadUrl={modalUrl ?? undefined} />
    </div>
  );
}

function EditionButton(props: { selected: boolean; onSelect: () => void; title: string; note: string; colorClass: string }) {
  const { selected, onSelect, title, note, colorClass } = props;
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      onClick={onSelect}
      className={`flex items-center gap-2 rounded-lg px-3.5 py-1.5 text-xs font-semibold transition-colors ${
        selected ? `${colorClass} text-white shadow-md` : "text-fg-muted hover:text-fg"
      }`}
    >
      <span>{title}</span>
      <span className="font-mono text-[10px] font-normal opacity-80">{note}</span>
    </button>
  );
}

function PlatformCard(props: { spec: PlatformSpec; full: boolean; getUrl: (file: string) => string; onDownload: (href: string) => void }) {
  const { spec, full, getUrl, onDownload } = props;
  const accent = full ? "text-done border-done-emphasis/40 hover:border-done" : "text-accent border-edge hover:border-accent";
  return (
    <section aria-labelledby={`dl-${spec.id}`} className="card flex flex-col justify-between gap-5 p-6">
      <div className="space-y-4">
        <div className="flex items-center gap-2.5">
          {spec.icon}
          <div>
            <h4 id={`dl-${spec.id}`} className="text-sm font-bold text-fg">
              {spec.title}
              {full && <span className="ml-2 rounded bg-done-emphasis/20 px-1.5 py-0.5 font-mono text-[10px] text-done">Full</span>}
            </h4>
            <p className="text-[11px] text-fg-muted">{spec.subtitle}</p>
          </div>
        </div>

        <div className="space-y-3">
          {spec.groups.map((group) => (
            <div key={group.title} className="space-y-1.5">
              <div className="flex items-center justify-between gap-2 text-[11px] font-semibold text-fg-muted">
                <span>{group.title}</span>
                {group.note && <span className="font-mono text-[10px] font-normal text-fg-subtle">{group.note}</span>}
              </div>
              <ul className="flex flex-wrap gap-2">
                {group.links.map((link, index) => {
                  const href = getUrl(link.file);
                  const first = index === 0;
                  return (
                    <li key={link.file}>
                      <a
                        href={href}
                        onClick={() => onDownload(href)}
                        className={`inline-flex items-center gap-1.5 rounded-md border bg-canvas-overlay px-2.5 py-1.5 text-xs font-semibold transition-colors hover:bg-canvas-hover ${
                          first ? accent : `border-edge hover:border-edge-hover ${link.minor ? "text-fg-muted" : "text-fg-secondary"}`
                        }`}
                      >
                        {first && <span className={`h-1.5 w-1.5 rounded-full ${full ? "bg-done" : "bg-accent"}`} />}
                        {link.label}
                      </a>
                    </li>
                  );
                })}
              </ul>
            </div>
          ))}
        </div>
      </div>
      <div className="border-t border-edge-muted pt-3 text-[11px] leading-relaxed text-fg-muted">{spec.footer}</div>
    </section>
  );
}
