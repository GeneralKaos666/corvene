import { useEffect, useRef } from "react";
import CopyButton from "./CopyButton";
import { BREW_COMMAND } from "../lib/github";

const XATTR_COMMAND = "xattr -d com.apple.quarantine /Applications/Corvene.app";

interface Props {
  open: boolean;
  onClose: () => void;
  downloadUrl?: string;
}

/**
 * Shown when a macOS download starts: Corvene is signed with a self-signed
 * certificate, so Gatekeeper blocks the first launch of a downloaded copy
 * (README › Install › macOS). A native <dialog> gives Escape, focus trapping
 * and an inert page for free.
 */
export default function MacQuarantineModal({ open, onClose, downloadUrl }: Props) {
  const ref = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);

  return (
    <dialog
      ref={ref}
      onClose={onClose}
      onClick={(e) => {
        // a click on the backdrop lands on the dialog element itself
        if (e.target === e.currentTarget) onClose();
      }}
      aria-labelledby="mac-modal-title"
      className="m-auto w-[calc(100%-2rem)] max-w-lg rounded-2xl border border-edge bg-canvas p-0 text-fg shadow-2xl backdrop:bg-black/70 backdrop:backdrop-blur-sm open:animate-[dialog-in_180ms_ease-out]"
    >
      <div className="space-y-5 p-6 sm:p-7">
        <div className="flex items-start justify-between gap-4">
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-xl border border-success-emphasis/30 bg-success-emphasis/15 text-success">
              <svg className="h-5 w-5" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
                <path d="M7.467.133a1.748 1.748 0 0 1 1.066 0l5.25 1.68A1.75 1.75 0 0 1 15 3.48V7c0 1.566-.32 3.182-1.303 4.682-.983 1.498-2.585 2.813-5.032 3.855a1.697 1.697 0 0 1-1.33 0c-2.447-1.042-4.049-2.357-5.032-3.855C1.32 10.182 1 8.566 1 7V3.48a1.75 1.75 0 0 1 1.217-1.667Zm.61 1.429a.25.25 0 0 0-.153 0l-5.25 1.68a.25.25 0 0 0-.174.238V7c0 1.358.275 2.666 1.057 3.86.784 1.194 2.121 2.34 4.366 3.297a.196.196 0 0 0 .154 0c2.245-.956 3.582-2.104 4.366-3.298C13.225 9.666 13.5 8.36 13.5 7V3.48a.251.251 0 0 0-.174-.237l-5.25-1.68ZM8.75 4.75v3a.75.75 0 0 1-1.5 0v-3a.75.75 0 0 1 1.5 0ZM9 10.5a1 1 0 1 1-2 0 1 1 0 0 1 2 0Z" />
              </svg>
            </div>
            <div>
              <h2 id="mac-modal-title" className="text-lg font-bold text-fg">
                Opening Corvene for the first time
              </h2>
              <p className="text-xs text-fg-muted">Your download has started</p>
            </div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-lg p-1.5 text-fg-muted transition-colors hover:bg-canvas-overlay hover:text-fg"
            aria-label="Close"
            autoFocus
          >
            <svg className="h-5 w-5" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
              <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.749.749 0 0 1 1.275.326.749.749 0 0 1-.215.734L9.06 8l3.22 3.22a.749.749 0 0 1-.326 1.275.749.749 0 0 1-.734-.215L8 9.06l-3.22 3.22a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z" />
            </svg>
          </button>
        </div>

        <p className="rounded-xl border border-edge/70 bg-canvas-subtle p-3.5 text-sm leading-relaxed text-fg-muted">
          Corvene is not notarized by Apple (there is no paid developer account behind it), so macOS blocks the first
          launch of a downloaded copy. Allow it once; updates install without asking.
        </p>

        <ol className="space-y-4 text-sm">
          <li className="space-y-1.5">
            <div className="flex items-center gap-2 font-semibold text-fg">
              <span className="flex h-5 w-5 items-center justify-center rounded-full border border-edge bg-canvas-overlay text-[11px] text-fg-secondary">1</span>
              Move <span className="font-mono text-[13px]">Corvene.app</span> to Applications and open it
            </div>
            <p className="pl-7 leading-relaxed text-fg-muted">
              macOS says it cannot verify the app. Click <strong className="text-fg-secondary">Done</strong>.
            </p>
          </li>
          <li className="space-y-1.5">
            <div className="flex items-center gap-2 font-semibold text-fg">
              <span className="flex h-5 w-5 items-center justify-center rounded-full border border-edge bg-canvas-overlay text-[11px] text-fg-secondary">2</span>
              Allow it in System Settings
            </div>
            <p className="pl-7 leading-relaxed text-fg-muted">
              Open <strong className="text-fg-secondary">System Settings › Privacy &amp; Security</strong>, scroll down and
              click <strong className="text-fg-secondary">Open Anyway</strong>. On macOS 14 and older you can instead
              right-click the app and choose Open.
            </p>
          </li>
        </ol>

        <div className="space-y-2 border-t border-edge-muted pt-4 text-sm">
          <p className="text-fg-muted">Or clear the quarantine flag from Terminal:</p>
          <div className="flex items-center justify-between gap-3 rounded-lg border border-edge bg-canvas-inset px-3 py-2 font-mono text-[12px] text-accent-muted">
            <code className="truncate">{XATTR_COMMAND}</code>
            <CopyButton text={XATTR_COMMAND} />
          </div>
          <p className="text-xs text-fg-subtle">
            Homebrew users skip all this: <code className="font-mono text-fg-muted">{BREW_COMMAND}</code> clears the
            flag on install.
          </p>
        </div>

        <div className="flex items-center justify-between gap-3 pt-1">
          {downloadUrl ? (
            <a href={downloadUrl} className="text-xs font-semibold text-accent hover:underline">
              Download did not start? Try again
            </a>
          ) : (
            <span />
          )}
          <button type="button" onClick={onClose} className="btn-primary px-5 py-2 text-xs">
            Got it
          </button>
        </div>
      </div>
    </dialog>
  );
}
