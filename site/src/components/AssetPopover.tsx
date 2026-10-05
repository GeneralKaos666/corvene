import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type React from "react";
import { createPortal } from "react-dom";
import type { ReleaseAsset } from "../lib/github";

const WIDTH = 264;
const GUTTER = 12;

export function formatSize(bytes: number): [string, string] {
  const units = ["B", "KB", "MB", "GB"];
  let size = bytes;
  let unit = 0;
  while (size >= 1024 && unit < units.length - 1) {
    size /= 1024;
    unit++;
  }
  return [size >= 100 || unit === 0 ? size.toFixed(0) : size.toFixed(1), units[unit]];
}

function kind(file: string): string {
  const lower = file.toLowerCase();
  if (lower.endsWith(".tar.gz")) return "tar.gz";
  if (lower.endsWith("-setup.exe")) return "installer";
  if (lower.endsWith("-portable.zip")) return "portable";
  const ext = lower.match(/\.([a-z0-9]+)$/);
  return ext ? (ext[1] === "appimage" ? "AppImage" : ext[1]) : "file";
}

interface Hovered {
  file: string;
  rect: DOMRect;
}

/**
 * Hover (and keyboard focus) cards for release download links: the asset's
 * size, download count, upload date and sha256, or a note that the release
 * does not carry it yet. Spread `handlers` on a container; every
 * `…/releases/download/…` link inside gets the card.
 */
export function useAssetPopover(assets: ReleaseAsset[], version: string, loaded: boolean) {
  const [hovered, setHovered] = useState<Hovered | null>(null);
  const [visible, setVisible] = useState(false);
  const hideTimer = useRef<number | undefined>(undefined);
  const anchor = useRef<HTMLAnchorElement | null>(null);
  const byName = useMemo(() => new Map(assets.map((a) => [a.name, a])), [assets]);

  const show = useCallback((target: EventTarget | null) => {
    const link = (target as Element | null)?.closest?.('a[href*="/releases/download/"]') as HTMLAnchorElement | null;
    if (!link) return;
    window.clearTimeout(hideTimer.current);
    anchor.current = link;
    const file = decodeURIComponent(new URL(link.href).pathname.split("/").pop() || "");
    setHovered({ file, rect: link.getBoundingClientRect() });
    requestAnimationFrame(() => setVisible(true));
  }, []);

  const hide = useCallback(() => {
    setVisible(false);
    anchor.current = null;
    window.clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => setHovered(null), 160);
  }, []);

  // the card is placed once: scrolling or resizing would leave it behind
  useEffect(() => {
    if (!hovered) return;
    const away = () => {
      window.clearTimeout(hideTimer.current);
      setVisible(false);
      setHovered(null);
    };
    window.addEventListener("scroll", away, { passive: true });
    window.addEventListener("resize", away);
    return () => {
      window.removeEventListener("scroll", away);
      window.removeEventListener("resize", away);
    };
  }, [hovered]);

  useEffect(() => () => window.clearTimeout(hideTimer.current), []);

  const handlers = {
    onMouseOver: (e: React.MouseEvent) => show(e.target),
    onMouseOut: (e: React.MouseEvent) => {
      const next = e.relatedTarget as Node | null;
      if (anchor.current && next && anchor.current.contains(next)) return;
      if ((e.target as Element).closest?.('a[href*="/releases/download/"]')) hide();
    },
    onFocus: (e: React.FocusEvent) => show(e.target),
    onBlur: () => hide(),
  };

  let popover: React.ReactNode = null;
  if (hovered && typeof document !== "undefined") {
    const { file, rect } = hovered;
    const asset = byName.get(file);
    const full = /^corvene-full/i.test(file);
    const center = rect.left + rect.width / 2;
    const left = Math.min(Math.max(center - WIDTH / 2, GUTTER), window.innerWidth - WIDTH - GUTTER);
    // above the link, unless that would leave the window (or slide under the sticky header)
    const above = rect.top > 190;
    const top = above ? rect.top - 10 : rect.bottom + 10;
    const arrow = Math.min(Math.max(center - left, 18), WIDTH - 18);
    const [size, unit] = asset?.size ? formatSize(asset.size) : ["", ""];
    const sha = asset?.digest?.startsWith("sha256:") ? asset.digest.slice(7) : "";
    const date = asset?.updated_at
      ? new Date(asset.updated_at).toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" })
      : "";

    popover = createPortal(
      <div
        role="tooltip"
        className="pointer-events-none fixed z-[100]"
        style={{ left, top, width: WIDTH, transform: above ? "translateY(-100%)" : undefined }}
      >
        <div
          className={`relative motion-safe:transition-[opacity,translate,scale] motion-safe:duration-150 motion-safe:ease-out ${
            visible ? "opacity-100 translate-y-0 scale-100" : `opacity-0 scale-[0.97] ${above ? "translate-y-1" : "-translate-y-1"}`
          }`}
          style={{ transformOrigin: `${arrow}px ${above ? "100%" : "0"}` }}
        >
          <div className="relative rounded-xl border border-edge bg-canvas-subtle/95 p-3.5 shadow-2xl shadow-black/60 ring-1 ring-white/[0.03] backdrop-blur-md">
            <span
              className={`absolute h-2.5 w-2.5 rotate-45 border-edge bg-canvas-subtle ${
                above ? "-bottom-[6px] border-b border-r" : "-top-[6px] border-l border-t"
              }`}
              style={{ left: arrow - 5 }}
            />

            <div className="flex items-center gap-1.5">
              <span
                className={`shrink-0 rounded-md px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide ${
                  full ? "bg-done-emphasis/20 text-done-muted" : "bg-accent-emphasis/20 text-accent-muted"
                }`}
              >
                {kind(file)}
              </span>
              {full && (
                <span className="shrink-0 rounded-md bg-done-emphasis/15 px-1.5 py-0.5 text-[10px] font-semibold text-done">
                  Full
                </span>
              )}
              <span className="min-w-0 truncate font-mono text-[11px] text-fg-muted" title={file}>
                {file}
              </span>
            </div>

            {asset ? (
              <>
                <div className="mt-2.5 flex items-baseline gap-1">
                  {asset.size ? (
                    <>
                      <span className="text-[26px] font-semibold leading-none tracking-tight text-fg tabular-nums">{size}</span>
                      <span className="text-sm font-medium text-fg-muted">{unit}</span>
                      <span className="ml-auto font-mono text-[10px] text-fg-subtle tabular-nums">
                        {asset.size.toLocaleString()} bytes
                      </span>
                    </>
                  ) : (
                    <span className="text-sm text-fg-muted">Size unknown</span>
                  )}
                </div>
                <div className="mt-2.5 flex items-center gap-3 border-t border-edge-muted pt-2.5 text-[11px] text-fg-muted">
                  {typeof asset.download_count === "number" && (
                    <span className="flex items-center gap-1">
                      <svg className="h-3 w-3" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
                        <path d="M2.75 14A1.75 1.75 0 0 1 1 12.25v-2.5a.75.75 0 0 1 1.5 0v2.5c0 .138.112.25.25.25h10.5a.25.25 0 0 0 .25-.25v-2.5a.75.75 0 0 1 1.5 0v2.5A1.75 1.75 0 0 1 13.25 14ZM7.25 7.689V2a.75.75 0 0 1 1.5 0v5.689l1.97-1.969a.749.749 0 1 1 1.06 1.06l-3.25 3.25a.749.749 0 0 1-1.06 0L4.22 6.78a.749.749 0 1 1 1.06-1.06l1.97 1.969Z" />
                      </svg>
                      {asset.download_count.toLocaleString()} {asset.download_count === 1 ? "download" : "downloads"}
                    </span>
                  )}
                  {date && <span className="ml-auto">{date}</span>}
                </div>
                {sha && (
                  <div className="mt-1.5 flex items-center gap-1.5 font-mono text-[10px] text-fg-subtle">
                    <span className="text-success">sha256</span>
                    <span className="truncate">{sha}</span>
                  </div>
                )}
              </>
            ) : (
              <div className="mt-2.5 flex items-start gap-2 text-[11px] leading-relaxed text-fg-muted">
                <span className="mt-1 h-1.5 w-1.5 shrink-0 rounded-full bg-attention" />
                <span>
                  {loaded ? (
                    <>
                      Not part of <span className="font-mono text-fg-secondary">v{version}</span>. The next release should
                      carry it.
                    </>
                  ) : (
                    "Size and checksum appear once the release list has loaded."
                  )}
                </span>
              </div>
            )}
          </div>
        </div>
      </div>,
      document.body
    );
  }

  return { handlers, popover };
}
