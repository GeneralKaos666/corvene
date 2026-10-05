import { useEffect, useState } from "react";
import MacQuarantineModal from "./MacQuarantineModal";
import CopyButton from "./CopyButton";
import { useAssetPopover } from "./AssetPopover";
import { assetNames } from "../lib/assets";
import { detectPlatform, type Platform } from "../lib/platform";
import { BREW_COMMAND, RELEASES_URL, assetUrl, fetchLatestRelease, type LatestRelease } from "../lib/github";

interface Props {
  /** the workspace version, used until the Releases API answers */
  fallbackVersion: string;
  /** the site's base path ("/corvene") for internal links */
  base: string;
}

interface Card {
  title: string;
  text: string;
  links: { label: string; href: string; mac?: boolean }[];
}

interface Variant {
  headline: string;
  button: string;
  buttonNote: string;
  href: string;
  mac?: boolean;
  cards: Card[];
}

/** The page's SoftwareApplication JSON-LD names the workspace version at build time; the live release wins. */
function updateStructuredDataVersion(version: string) {
  for (const script of document.querySelectorAll<HTMLScriptElement>('script[type="application/ld+json"]')) {
    try {
      const data = JSON.parse(script.textContent || "null");
      const items: Record<string, unknown>[] = Array.isArray(data) ? data : [data];
      let changed = false;
      for (const item of items) {
        if (item && item["@type"] === "SoftwareApplication" && item.softwareVersion !== version) {
          item.softwareVersion = version;
          changed = true;
        }
      }
      if (changed) script.textContent = JSON.stringify(data);
    } catch {
      // not ours
    }
  }
}

export default function HeroDownload({ fallbackVersion, base }: Props) {
  // "other" on the server and for the first client frame, so the markup
  // hydrates cleanly; the visitor's platform replaces it right after
  const [platform, setPlatform] = useState<Platform>("other");
  const [release, setRelease] = useState<LatestRelease | null>(null);
  const [modalUrl, setModalUrl] = useState<string | null>(null);

  useEffect(() => {
    setPlatform(detectPlatform());
    fetchLatestRelease().then((latest) => {
      setRelease(latest);
      if (latest) updateStructuredDataVersion(latest.version);
    });
  }, []);

  const version = release?.version ?? fallbackVersion;
  const assets = release?.assets ?? [];
  const url = (file: string) => assetUrl(version, file, assets);
  const names = assetNames(version, "standard");
  const { handlers, popover } = useAssetPopover(assets, version, release !== null);

  const windowsCard: Card = {
    title: "Windows",
    text: "A per-user installer that updates itself, a machine-wide .msi and a portable zip.",
    links: [
      { label: "Installer (x64)", href: url(names.windows.setup("x86_64")) },
      { label: "ARM64", href: url(names.windows.setup("aarch64")) },
    ],
  };
  const macCard: Card = {
    title: "macOS",
    text: "One universal app for Apple Silicon and Intel, or a smaller download for your chip.",
    links: [
      { label: "Universal .dmg", href: url(names.macos.dmg("universal")), mac: true },
      { label: "Apple Silicon", href: url(names.macos.dmg("arm64")), mac: true },
      { label: "Intel", href: url(names.macos.dmg("x86_64")), mac: true },
    ],
  };
  const linuxCard: Card = {
    title: "Linux",
    text: ".deb, .rpm, a self-updating AppImage, Flatpak, snap and a tarball, on x86_64 and arm64.",
    links: [
      { label: ".deb", href: url(names.linux.deb("amd64")) },
      { label: ".rpm", href: url(names.linux.rpm("x86_64")) },
      { label: "AppImage", href: url(names.linux.appimage("x86_64")) },
      { label: "All Linux packages", href: "#downloads" },
    ],
  };
  const androidCard: Card = {
    title: "Android",
    text: "A new app built for phones, and Corvene Legacy, the desktop app for tablets and Chromebooks. Experimental.",
    links: [
      { label: "arm64 APK", href: url(names.android.apk("arm64")) },
      { label: "Legacy APK", href: url(names.android.apk("arm64", "legacy")) },
    ],
  };

  const variants: Record<Platform, Variant> = {
    macos: {
      headline: "Download Corvene for macOS",
      button: "Download for macOS",
      buttonNote: "Universal .dmg · Apple Silicon and Intel",
      href: url(names.macos.dmg("universal")),
      mac: true,
      cards: [
        {
          title: "Smaller download for your Mac",
          text: "The universal app carries both architectures. These carry one.",
          links: [
            { label: "Apple Silicon .dmg", href: url(names.macos.dmg("arm64")), mac: true },
            { label: "Intel .dmg", href: url(names.macos.dmg("x86_64")), mac: true },
          ],
        },
        windowsCard,
        { ...linuxCard, title: "Linux and Android", links: [...linuxCard.links.slice(0, 2), androidCard.links[0], { label: "All packages", href: "#downloads" }] },
      ],
    },
    windows: {
      headline: "Download Corvene for Windows",
      button: "Download for Windows",
      buttonNote: "Installer · 64-bit x86",
      href: url(names.windows.setup("x86_64")),
      cards: [
        {
          title: "Windows on ARM, .msi or portable",
          text: "A native ARM64 build for Snapdragon PCs, a machine-wide .msi for deployment, and a zip that runs from anywhere.",
          links: [
            { label: "ARM64 installer", href: url(names.windows.setup("aarch64")) },
            { label: "x64 .msi", href: url(names.windows.msi("x86_64")) },
            { label: "Portable zip", href: url(names.windows.portable("x86_64")) },
          ],
        },
        macCard,
        { ...linuxCard, title: "Linux and Android", links: [...linuxCard.links.slice(0, 2), androidCard.links[0], { label: "All packages", href: "#downloads" }] },
      ],
    },
    linux: {
      headline: "Download Corvene for Linux",
      button: "Download the .deb",
      buttonNote: "Debian, Ubuntu, Mint · x86_64",
      href: url(names.linux.deb("amd64")),
      cards: [
        {
          title: "Other distributions",
          text: "An .rpm for Fedora and openSUSE, a self-updating AppImage for any distribution, Flatpak, snap and a tarball for Arch.",
          links: [
            { label: ".rpm", href: url(names.linux.rpm("x86_64")) },
            { label: "AppImage", href: url(names.linux.appimage("x86_64")) },
            { label: "arm64 and more", href: "#downloads" },
          ],
        },
        macCard,
        windowsCard,
      ],
    },
    android: {
      headline: "Download Corvene for Android",
      button: "Download the APK",
      buttonNote: "The new app for phones · arm64",
      href: url(names.android.apk("arm64")),
      cards: [
        {
          title: "Corvene Legacy",
          text: "The desktop app itself, GitHub Desktop 1:1, for tablets, Chromebooks and desktop modes. Installs next to the new one.",
          links: [
            { label: "arm64 APK", href: url(names.android.apk("arm64", "legacy")) },
            { label: "x86_64 (Chromebooks)", href: url(names.android.apk("x86_64", "legacy")) },
            { label: "Other devices", href: "#pick" },
          ],
        },
        { ...macCard, title: "Desktop", text: "The same app on macOS, Windows and Linux.", links: [macCard.links[0], windowsCard.links[0], linuxCard.links[0]] },
        {
          title: "What is inside",
          text: "git, OpenSSH and Git LFS are bundled, so nothing else needs installing. A universal APK and 32-bit builds are on the release.",
          links: [{ label: "Android notes", href: `${base}/docs/installation#android` }],
        },
      ],
    },
    other: {
      headline: "Download Corvene",
      button: "Download for macOS",
      buttonNote: "Universal .dmg · Apple Silicon and Intel",
      href: url(names.macos.dmg("universal")),
      mac: true,
      cards: [windowsCard, linuxCard, androidCard],
    },
  };

  const current = variants[platform];

  const onDownload = (href: string, mac?: boolean) => {
    if (mac) setModalUrl(href);
  };

  return (
    <div className="mx-auto w-full max-w-5xl" {...handlers}>
      {popover}

      <div className="mx-auto max-w-3xl text-center">
        <h1 className="font-display text-4xl font-bold tracking-tight text-fg sm:text-5xl lg:text-6xl">{current.headline}</h1>
        <p className="mt-5 text-base leading-relaxed text-fg-muted sm:text-lg">
          A fast, low-memory recreation of GitHub Desktop written in Rust. The same layout, menus, dialogs and
          shortcuts, with a native GPU-rendered engine instead of Electron.
        </p>
      </div>

      <div className="mt-10 flex flex-col items-center gap-3">
        <a href={current.href} onClick={() => onDownload(current.href, current.mac)} className="btn-hero">
          <svg className="h-5 w-5" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
            <path d="M2.75 14A1.75 1.75 0 0 1 1 12.25v-2.5a.75.75 0 0 1 1.5 0v2.5c0 .138.112.25.25.25h10.5a.25.25 0 0 0 .25-.25v-2.5a.75.75 0 0 1 1.5 0v2.5A1.75 1.75 0 0 1 13.25 14ZM7.25 7.689V2a.75.75 0 0 1 1.5 0v5.689l1.97-1.969a.749.749 0 1 1 1.06 1.06l-3.25 3.25a.749.749 0 0 1-1.06 0L4.22 6.78a.749.749 0 1 1 1.06-1.06l1.97 1.969Z" />
          </svg>
          <span>{current.button}</span>
        </a>
        <p className="text-xs text-fg-muted">
          {current.buttonNote} · <span className="font-mono">v{version}</span> ·{" "}
          <a href="#downloads" className="text-accent hover:underline">
            all platforms and formats
          </a>{" "}
          ·{" "}
          <a href="#pick" className="text-accent hover:underline">
            help me pick
          </a>
        </p>

        {(platform === "macos" || platform === "linux" || platform === "other") && (
          <div className="mt-3 flex flex-col items-center gap-2 sm:flex-row">
            <span className="text-xs text-fg-muted">Or with Homebrew:</span>
            <div className="flex max-w-full items-center gap-2 rounded-lg border border-edge bg-canvas-subtle py-1.5 pl-3 pr-1.5 font-mono text-xs text-accent-muted">
              <span className="select-none text-fg-subtle" aria-hidden="true">
                $
              </span>
              <code className="truncate">{BREW_COMMAND}</code>
              <CopyButton text={BREW_COMMAND} />
            </div>
          </div>
        )}
      </div>

      <div className="mt-14 grid grid-cols-1 gap-5 text-left md:grid-cols-3">
        {current.cards.map((card) => (
          <div key={card.title} className="card flex flex-col justify-between gap-4 p-6 transition-colors hover:border-edge-hover">
            <div>
              <h2 className="text-sm font-bold text-fg">{card.title}</h2>
              <p className="mt-1.5 text-xs leading-relaxed text-fg-muted">{card.text}</p>
            </div>
            <ul className="flex flex-wrap gap-x-4 gap-y-1.5 text-xs font-semibold">
              {card.links.map((link) => (
                <li key={link.label}>
                  <a href={link.href} onClick={() => onDownload(link.href, link.mac)} className="text-accent hover:underline">
                    {link.label} →
                  </a>
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>

      <p className="mt-8 text-center text-xs text-fg-subtle">
        Every release is on{" "}
        <a href={release?.htmlUrl ?? RELEASES_URL} className="text-fg-muted hover:text-fg hover:underline" target="_blank" rel="noopener noreferrer">
          GitHub Releases
        </a>
        , with sha256 checksums next to each file.
      </p>

      <MacQuarantineModal open={modalUrl !== null} onClose={() => setModalUrl(null)} downloadUrl={modalUrl ?? undefined} />
    </div>
  );
}
