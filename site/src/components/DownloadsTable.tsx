import { useEffect, useState, type ReactNode } from "react";
import MacQuarantineModal from "./MacQuarantineModal";
import CopyButton from "./CopyButton";
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
      subtitle: "Android 8.0 or newer. git, OpenSSH and Git LFS are bundled.",
      groups: [
        {
          title: full ? "APK (one per architecture)" : "APK",
          note: "Experimental",
          links: [
            { label: "arm64 (most phones)", file: names.android.apk("arm64") },
            ...(full ? [] : [{ label: "Universal (every ABI)", file: names.android.apk("universal") }]),
            { label: "armv7 (32-bit phones)", file: names.android.apk("armv7") },
            { label: "x86_64 (Chromebooks)", file: names.android.apk("x86_64"), minor: true },
            { label: "x86", file: names.android.apk("x86"), minor: true },
          ],
        },
      ],
      footer: <p>Open the APK on the device to install. Installing a newer one keeps repositories, accounts and settings.</p>,
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

function AppleIcon() {
  return (
    <svg className="h-5 w-5 shrink-0 text-fg" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M18.71 19.5c-.83 1.24-1.71 2.45-3.05 2.47-1.34.03-1.77-.79-3.29-.79-1.53 0-2 .77-3.27.82-1.31.05-2.3-1.32-3.14-2.53C4.25 17 2.94 12.45 4.7 9.39c.87-1.52 2.43-2.48 4.12-2.51 1.28-.02 2.5.87 3.29.87.78 0 2.26-1.07 3.81-.91.65.03 2.47.26 3.64 1.98-.09.06-2.17 1.28-2.15 3.81.03 3.02 2.65 4.03 2.68 4.04-.03.07-.42 1.44-1.38 2.83M15.97 6.37c.61-.75 1.04-1.8 0.91-2.87-.9.04-2.02.61-2.67 1.37-.58.67-1.1 1.74-.95 2.79 1.01.08 2.07-.52 2.71-1.29z" />
    </svg>
  );
}

function WindowsIcon() {
  return (
    <svg className="h-5 w-5 shrink-0 text-accent" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M0 3.449L9.75 2.1v9.451H0m10.949-9.602L24 0v11.4H10.949M0 12.6h9.75v9.451L0 20.699M10.949 12.6H24V24l-12.9-1.801" />
    </svg>
  );
}

function LinuxIcon() {
  // a terminal glyph stands in for the many distributions
  return (
    <svg className="h-5 w-5 shrink-0 text-attention-bright" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M0 2.75C0 1.784.784 1 1.75 1h12.5c.966 0 1.75.784 1.75 1.75v10.5A1.75 1.75 0 0 1 14.25 15H1.75A1.75 1.75 0 0 1 0 13.25Zm1.75-.25a.25.25 0 0 0-.25.25v10.5c0 .138.112.25.25.25h12.5a.25.25 0 0 0 .25-.25V2.75a.25.25 0 0 0-.25-.25ZM7.25 8a.749.749 0 0 1-.22.53l-2.25 2.25a.749.749 0 0 1-1.275-.326.749.749 0 0 1 .215-.734L5.44 8 3.72 6.28a.749.749 0 0 1 .326-1.275.749.749 0 0 1 .734.215l2.25 2.25c.141.14.22.331.22.53Zm1.5 1.5h3a.75.75 0 0 1 0 1.5h-3a.75.75 0 0 1 0-1.5Z" />
    </svg>
  );
}

function AndroidIcon() {
  return (
    <svg className="h-5 w-5 shrink-0 text-success" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
      <path d="M17.523 15.3414c-.5511 0-.9993-.4486-.9993-.9997s.4482-.9993.9993-.9993c.551 0 .9993.4482.9993.9993.0001.5511-.4483.9997-.9993.9997m-11.046 0c-.5511 0-.9993-.4486-.9993-.9997s.4482-.9993.9993-.9993c.5511 0 .9993.4482.9993.9993 0 .5511-.4482.9997-.9993.9997m11.4045-6.02l1.9973-3.4592a.416.416 0 00-.1521-.5676.416.416 0 00-.5676.1521l-2.0223 3.503C15.5898 8.4069 13.8532 8.0827 12 8.0827s-3.5898.3242-5.1368.867l-2.0223-3.503a.416.416 0 00-.5676-.1521.416.416 0 00-.1521.5676l1.9973 3.4592C2.6889 11.1867.3432 14.6589 0 18.761h24c-.3432-4.1021-2.6889-7.5743-6.1185-9.4396" />
    </svg>
  );
}
