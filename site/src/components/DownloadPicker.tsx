import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import type React from "react";
import CopyButton from "./CopyButton";
import MacQuarantineModal from "./MacQuarantineModal";
import { formatSize } from "./AssetPopover";
import { AndroidIcon, AppleIcon, LinuxIcon, WindowsIcon } from "./PlatformIcons";
import type { Edition } from "../lib/assets";
import { BREW_COMMAND, assetUrl, fetchLatestRelease, type LatestRelease, type ReleaseAsset } from "../lib/github";
import { detectArch, detectPlatform, isChromeOS, isIOS, type Platform } from "../lib/platform";
import { useCopy } from "../lib/useCopy";
import {
  ARCH_CHECK,
  ARCH_OPTIONS,
  ARCH_QUESTION,
  EDITION_FACTS,
  FORMAT_OPTIONS,
  FORMAT_QUESTION,
  OS_OPTIONS,
  archLabel,
  estimateMB,
  installSteps,
  resolveFile,
  type ArchId,
  type FormatId,
  type FormatOption,
  type OsId,
} from "../data/picker";

type Step = "os" | "arch" | "format" | "edition" | "result";

const STEP_LABEL: Record<Exclude<Step, "result">, string> = {
  os: "System",
  arch: "Processor",
  format: "Package",
  edition: "Edition",
};

/** how long a choice stays on screen, checked, before the next question */
const ADVANCE_MS = 260;

interface Answers {
  os: OsId | null;
  arch: ArchId | null;
  format: FormatId | null;
  edition: Edition | null;
}

const EMPTY: Answers = { os: null, arch: null, format: null, edition: null };

function formatOf(answers: Answers): FormatOption | undefined {
  return answers.os && answers.format ? FORMAT_OPTIONS[answers.os].find((f) => f.id === answers.format) : undefined;
}

function stepsFor(answers: Answers): Step[] {
  return formatOf(answers)?.standardOnly ? ["os", "arch", "format", "result"] : ["os", "arch", "format", "edition", "result"];
}

function osIcon(os: OsId, className: string) {
  switch (os) {
    case "macos":
      return <AppleIcon className={`${className} text-fg`} />;
    case "windows":
      return <WindowsIcon className={`${className} text-accent`} />;
    case "linux":
      return <LinuxIcon className={`${className} text-attention-bright`} />;
    case "android":
      return <AndroidIcon className={`${className} text-success`} />;
  }
}

/**
 * "Help me pick": a short questionnaire (system, processor, package,
 * edition) that ends at the one release file to download and how to
 * install it. Opens from its button above the downloads list and from any
 * `#pick` link on the page.
 */
export default function DownloadPicker({ fallbackVersion }: { fallbackVersion: string }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const body = useRef<HTMLDivElement>(null);
  const stepRoot = useRef<HTMLDivElement>(null);
  const advanceTimer = useRef<number | undefined>(undefined);

  const [open, setOpen] = useState(false);
  const [answers, setAnswers] = useState<Answers>(EMPTY);
  const [step, setStep] = useState<Step>("os");
  const [direction, setDirection] = useState<1 | -1>(1);
  const [detectedOs, setDetectedOs] = useState<Platform>("other");
  const [detectedArch, setDetectedArch] = useState<ArchId | null>(null);
  const [device, setDevice] = useState({ chromeOS: false, ios: false });
  const [release, setRelease] = useState<LatestRelease | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [macUrl, setMacUrl] = useState<string | null>(null);

  const version = release?.version ?? fallbackVersion;
  const assets = release?.assets ?? [];
  const steps = stepsFor(answers);
  // Homebrew installs the Standard edition: implied, never stored, so another
  // package still asks
  const edition: Edition | null = formatOf(answers)?.standardOnly ? "standard" : answers.edition;
  const questions = steps.filter((s) => s !== "result");
  const stepIndex = steps.indexOf(step);

  // #pick links (the hero's "help me pick") open the guide
  useEffect(() => {
    const fromHash = () => {
      if (window.location.hash === "#pick") setOpen(true);
    };
    fromHash();
    window.addEventListener("hashchange", fromHash);
    return () => window.removeEventListener("hashchange", fromHash);
  }, []);

  useEffect(() => {
    const el = dialog.current;
    if (!el) return;
    if (open && !el.open) {
      el.showModal();
      const platform = detectPlatform();
      setDetectedOs(platform);
      setDevice({ chromeOS: isChromeOS(), ios: isIOS() });
      detectArch(platform).then(setDetectedArch);
      fetchLatestRelease().then((latest) => {
        setRelease(latest);
        setLoaded(true);
      });
    }
    if (!open && el.open) el.close();
    // the page behind a modal should not scroll under a finger
    document.documentElement.style.overflow = open ? "hidden" : "";
    return () => {
      document.documentElement.style.overflow = "";
    };
  }, [open]);

  useEffect(() => () => window.clearTimeout(advanceTimer.current), []);

  const close = useCallback(() => {
    window.clearTimeout(advanceTimer.current);
    setOpen(false);
    if (window.location.hash === "#pick") history.replaceState(null, "", window.location.pathname + window.location.search);
  }, []);

  const goTo = useCallback(
    (next: Step) => {
      window.clearTimeout(advanceTimer.current);
      const order: Step[] = ["os", "arch", "format", "edition", "result"];
      setDirection(order.indexOf(next) >= order.indexOf(step) ? 1 : -1);
      setStep(next);
    },
    [step]
  );

  // a new step: back to the top, and focus on its chosen (or first) option
  useEffect(() => {
    if (!open) return;
    body.current?.scrollTo({ top: 0 });
    const frame = requestAnimationFrame(() => {
      const root = stepRoot.current;
      const target =
        root?.querySelector<HTMLElement>('[role="radio"][aria-checked="true"]') ??
        root?.querySelector<HTMLElement>('[role="radio"]:not([disabled])') ??
        root?.querySelector<HTMLElement>("[data-autofocus]");
      target?.focus({ preventScroll: true });
    });
    return () => cancelAnimationFrame(frame);
  }, [step, open]);

  const answer = (patch: Partial<Answers>) => {
    const next = { ...answers, ...patch };
    // a new system starts the rest over
    if (patch.os && patch.os !== answers.os) Object.assign(next, { arch: null, format: null, edition: null });
    // a package not built for the new processor is dropped
    if (patch.arch && next.os && next.format) {
      const format = FORMAT_OPTIONS[next.os].find((f) => f.id === next.format);
      if (format?.archs && !format.archs.includes(patch.arch)) next.format = null;
    }
    if (next.os === "android" && next.arch === "universal" && next.edition === "full") next.edition = null;
    setAnswers(next);

    // always the next question, even one answered before: a changed answer
    // never skips past a later one (its earlier choice stays selected)
    const order = stepsFor(next);
    const upcoming = order[order.indexOf(step) + 1];
    window.clearTimeout(advanceTimer.current);
    const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    advanceTimer.current = window.setTimeout(() => {
      setDirection(1);
      setStep(upcoming);
    }, reduce ? 0 : ADVANCE_MS);
  };

  const back = () => {
    if (stepIndex > 0) goTo(steps[stepIndex - 1]);
  };

  const startOver = () => {
    setAnswers(EMPTY);
    goTo("os");
  };

  const canAdvance = step !== "result" && answers[step as keyof Answers] !== null;

  const onKeyDown = (e: React.KeyboardEvent) => {
    const target = e.target as HTMLElement;
    if (target.closest("input, textarea")) return;
    const radios = Array.from(stepRoot.current?.querySelectorAll<HTMLButtonElement>('[role="radio"]:not([disabled])') ?? []);
    if (/^[1-9]$/.test(e.key) && !e.metaKey && !e.ctrlKey && !e.altKey) {
      const radio = radios[Number(e.key) - 1];
      if (radio) {
        e.preventDefault();
        radio.focus();
        radio.click();
      }
      return;
    }
    if (["ArrowDown", "ArrowRight", "ArrowUp", "ArrowLeft"].includes(e.key) && target.getAttribute("role") === "radio") {
      e.preventDefault();
      const at = radios.indexOf(target as HTMLButtonElement);
      const delta = e.key === "ArrowDown" || e.key === "ArrowRight" ? 1 : -1;
      radios[(at + delta + radios.length) % radios.length]?.focus();
      return;
    }
    if ((e.key === "Backspace" && target.tagName !== "INPUT") || (e.altKey && e.key === "ArrowLeft")) {
      if (stepIndex > 0) {
        e.preventDefault();
        back();
      }
    }
  };

  const size = (file: string | null) => (file ? assets.find((a) => a.name === file) : undefined);

  const question = (() => {
    switch (step) {
      case "os":
        return { title: "Which system will Corvene run on?", lead: "Pick the device you will install it on. You can change any answer later." };
      case "arch":
        return answers.os ? ARCH_QUESTION[answers.os] : null;
      case "format":
        return answers.os ? FORMAT_QUESTION[answers.os] : null;
      case "edition":
        return {
          title: "Standard or Full?",
          lead: "Same app. The only difference is whether the optional tree-sitter grammars are inside or downloaded on demand.",
        };
      case "result":
        return { title: "Here's your download", lead: "" };
    }
  })();

  return (
    <>
      <button type="button" onClick={() => setOpen(true)} className="picker-trigger group" aria-haspopup="dialog">
        <span className="flex h-6 w-6 items-center justify-center rounded-full bg-gradient-to-br from-accent-emphasis to-done-emphasis text-white shadow-[0_0_14px_-2px] shadow-accent-emphasis/70">
          <SparkleIcon className="h-3.5 w-3.5" />
        </span>
        <span className="text-fg">Help me pick</span>
        <span className="hidden text-fg-muted sm:inline">· four quick questions</span>
        <ChevronRight className="h-3.5 w-3.5 text-fg-muted transition-transform group-hover:translate-x-0.5 group-hover:text-fg" />
      </button>

      <dialog
        ref={dialog}
        onClose={close}
        onClick={(e) => {
          if (e.target === e.currentTarget) close();
        }}
        onKeyDown={onKeyDown}
        aria-labelledby="picker-question"
        className="picker-dialog text-left mt-auto mb-0 max-h-[92dvh] w-full max-w-none flex-col overflow-hidden rounded-t-3xl border border-b-0 border-edge bg-canvas p-0 text-fg shadow-2xl shadow-black backdrop:bg-black/70 backdrop:backdrop-blur-sm open:flex sm:m-auto sm:max-h-[min(88dvh,860px)] sm:w-[calc(100%-2rem)] sm:max-w-[46rem] sm:rounded-2xl sm:border-b"
      >
        {/* header: progress and the answers so far */}
        <div className="relative shrink-0 border-b border-edge-muted bg-canvas-subtle/40 px-5 pb-4 pt-3 sm:px-7 sm:pt-5">
          <div className="mx-auto mb-3 h-1 w-10 rounded-full bg-edge sm:hidden" aria-hidden="true" />
          <div className="flex items-center justify-between gap-4">
            <div className="flex items-center gap-2 text-xs font-semibold text-fg-muted">
              <SparkleIcon className="h-3.5 w-3.5 text-done" />
              <span className="eyebrow text-fg-secondary">Help me pick</span>
              <span className="text-fg-subtle" aria-hidden="true">
                ·
              </span>
              <span className="tabular-nums">
                {step === "result" ? "Done" : `Question ${questions.indexOf(step) + 1} of ${questions.length}`}
              </span>
            </div>
            <button
              type="button"
              onClick={close}
              className="-mr-1.5 rounded-lg p-1.5 text-fg-muted transition-colors hover:bg-canvas-overlay hover:text-fg"
              aria-label="Close"
            >
              <CloseIcon className="h-5 w-5" />
            </button>
          </div>

          <div className="mt-3 flex gap-1.5" aria-hidden="true">
            {questions.map((s, i) => {
              const done = step === "result" || i < questions.indexOf(step);
              const current = s === step;
              return (
                <span key={s} className="relative h-1 flex-1 overflow-hidden rounded-full bg-edge-muted">
                  <span
                    className={`absolute inset-y-0 left-0 rounded-full transition-[width,background-color] duration-500 ease-out ${
                      done ? "w-full bg-gradient-to-r from-accent-emphasis to-done-emphasis" : current ? "w-1/2 bg-accent/70" : "w-0"
                    }`}
                  />
                </span>
              );
            })}
          </div>

          <nav aria-label="Your answers" className="mt-3 flex min-h-7 flex-wrap items-center gap-1.5">
            {questions.map((s) => {
              const value = chipValue(s, answers);
              if (!value) return null;
              return (
                <button
                  key={s}
                  type="button"
                  onClick={() => goTo(s)}
                  aria-current={s === step ? "step" : undefined}
                  title={`Change ${STEP_LABEL[s as Exclude<Step, "result">].toLowerCase()}`}
                  className={`picker-chip-in inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-medium transition-colors ${
                    s === step
                      ? "border-accent/60 bg-accent-emphasis/15 text-fg"
                      : "border-edge bg-canvas-overlay/60 text-fg-secondary hover:border-edge-hover hover:text-fg"
                  }`}
                >
                  <span className="text-fg-subtle">{STEP_LABEL[s as Exclude<Step, "result">]}</span>
                  {value}
                </button>
              );
            })}
            {!answers.os && <span className="text-[11px] text-fg-subtle">Your answers will collect here.</span>}
          </nav>
        </div>

        {/* body: the current question */}
        <AutoHeight scrollRef={body}>
          <div
            key={step}
            ref={stepRoot}
            className={`px-5 py-6 sm:px-7 sm:py-7 ${direction === 1 ? "picker-step-next" : "picker-step-prev"}`}
          >
            {question && step !== "result" && (
              <div className="mb-5">
                <h2 id="picker-question" className="font-display text-xl font-bold tracking-tight text-fg sm:text-2xl">
                  {question.title}
                </h2>
                {question.lead && <p className="mt-1.5 text-sm leading-relaxed text-fg-muted">{question.lead}</p>}
              </div>
            )}
            <p className="sr-only" aria-live="polite">
              {step === "result" ? "Your download is ready." : question?.title}
            </p>

            {step === "os" && (
              <OsStep answers={answers} detected={detectedOs} chromeOS={device.chromeOS} ios={device.ios} onPick={(os) => answer({ os })} />
            )}
            {step === "arch" && answers.os && (
              <ArchStep
                os={answers.os}
                selected={answers.arch}
                detected={answers.os === detectedOs ? detectedArch : null}
                onPick={(arch) => answer({ arch })}
              />
            )}
            {step === "format" && answers.os && answers.arch && (
              <FormatStep os={answers.os} arch={answers.arch} selected={answers.format} onPick={(format) => answer({ format })} />
            )}
            {step === "edition" && answers.os && answers.arch && answers.format && (
              <EditionStep
                answers={answers}
                version={version}
                assets={assets}
                onPick={(edition) => answer({ edition })}
                onPickArm64={() => {
                  setAnswers({ ...answers, arch: "arm64", edition: "full" });
                  goTo("result");
                }}
              />
            )}
            {step === "result" && answers.os && answers.arch && answers.format && edition && (
              <ResultStep
                answers={{ os: answers.os, arch: answers.arch, format: answers.format, edition }}
                version={version}
                asset={size(resolveFile(version, edition, answers.os, answers.arch, answers.format))}
                loaded={loaded}
                onDownload={(href) => answers.os === "macos" && setMacUrl(href)}
                onSwitchApp={() => {
                  setAnswers({ ...answers, format: answers.format === "compose" ? "legacy" : "compose" });
                  setDirection(1);
                }}
                onShowAll={() => {
                  close();
                  document.getElementById("downloads")?.scrollIntoView({ behavior: "smooth" });
                }}
              />
            )}
          </div>
        </AutoHeight>

        {/* footer: back, next, keyboard hint */}
        <div className="flex shrink-0 items-center justify-between gap-3 border-t border-edge-muted bg-canvas-subtle/40 px-5 py-3 pb-[max(0.75rem,env(safe-area-inset-bottom))] sm:px-7">
          <button
            type="button"
            onClick={back}
            disabled={stepIndex === 0}
            className="btn-invisible -ml-2 px-2.5 py-1.5 text-xs disabled:pointer-events-none disabled:opacity-0"
          >
            <ChevronLeft className="h-3.5 w-3.5" />
            Back
          </button>
          <span className="hidden items-center gap-1.5 text-[11px] text-fg-subtle md:flex">
            {step !== "result" && (
              <>
                <span className="kbd">1</span>–<span className="kbd">{optionCount(step, answers)}</span> to choose
                <span className="mx-1">·</span>
                <span className="kbd">⌫</span> back
                <span className="mx-1">·</span>
                <span className="kbd">esc</span> close
              </>
            )}
          </span>
          {step === "result" ? (
            <button type="button" onClick={startOver} className="btn-default px-3 py-1.5 text-xs">
              Start over
            </button>
          ) : (
            <button
              type="button"
              onClick={() => goTo(steps[stepIndex + 1])}
              disabled={!canAdvance}
              className="btn-default px-3 py-1.5 text-xs disabled:pointer-events-none disabled:opacity-40"
            >
              Next
              <ChevronRight className="h-3.5 w-3.5" />
            </button>
          )}
        </div>
      </dialog>

      <MacQuarantineModal open={macUrl !== null} onClose={() => setMacUrl(null)} downloadUrl={macUrl ?? undefined} />
    </>
  );
}

function chipValue(step: Step, answers: Answers): string | null {
  const { os, arch, format, edition } = answers;
  switch (step) {
    case "os":
      return os ? (OS_OPTIONS.find((o) => o.id === os)?.title ?? null) : null;
    case "arch":
      return os && arch ? archLabel(os, arch) : null;
    case "format": {
      const f = formatOf(answers);
      return f ? formatLabel(f) : null;
    }
    case "edition":
      return edition ? (edition === "full" ? "Full" : "Standard") : null;
  }
  return null;
}

/** ".dmg", ".deb"…; a name where the extension says little */
function formatLabel(format: FormatOption): string {
  return format.tag.startsWith(".") && format.id !== "portable" ? format.tag : format.title;
}

function optionCount(step: Step, answers: Answers): number {
  if (step === "os") return OS_OPTIONS.length;
  if (step === "arch" && answers.os) return ARCH_OPTIONS[answers.os].length;
  if (step === "format" && answers.os) return FORMAT_OPTIONS[answers.os].length;
  return 2;
}

/** The dialog's scrolling body, its height eased between steps. */
function AutoHeight({ children, scrollRef }: { children: ReactNode; scrollRef: React.RefObject<HTMLDivElement | null> }) {
  const inner = useRef<HTMLDivElement>(null);
  const [height, setHeight] = useState<number | undefined>(undefined);
  // no easing from 0: the closed dialog measures nothing
  const [eased, setEased] = useState(false);

  useLayoutEffect(() => {
    const el = inner.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => {
      const next = el.offsetHeight;
      setHeight((previous) => {
        setEased(Boolean(previous) && Boolean(next));
        return next || undefined;
      });
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  return (
    <div
      ref={scrollRef}
      className={`min-h-0 shrink overflow-y-auto overflow-x-hidden overscroll-contain ${
        eased ? "motion-safe:transition-[height] motion-safe:duration-300 motion-safe:ease-[cubic-bezier(0.2,0.8,0.2,1)]" : ""
      }`}
      style={{ height }}
    >
      <div ref={inner}>{children}</div>
    </div>
  );
}

/* ---------- steps ---------- */

function OsStep(props: { answers: Answers; detected: Platform; chromeOS: boolean; ios: boolean; onPick: (os: OsId) => void }) {
  const { answers, detected, chromeOS, ios, onPick } = props;
  return (
    <div className="space-y-4">
      <div role="radiogroup" aria-labelledby="picker-question" className="grid grid-cols-2 gap-3">
        {OS_OPTIONS.map((os, i) => {
          const selected = answers.os === os.id;
          const here = detected === os.id;
          return (
            <button
              key={os.id}
              type="button"
              role="radio"
              aria-checked={selected}
              onClick={() => onPick(os.id)}
              className={`group picker-option flex flex-col items-start gap-3 p-4 text-left sm:p-5 ${selected ? "is-selected" : ""}`}
            >
              <div className="flex w-full items-start justify-between gap-2">
                <span className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl border border-edge bg-canvas-overlay/70 transition-transform group-hover:scale-105">
                  {osIcon(os.id, "h-6 w-6")}
                </span>
                <span className="flex items-center gap-1.5">
                  {here && <Badge tone="accent">{chromeOS && os.id === "android" ? "Chromebook" : "This device"}</Badge>}
                  <SelectMark selected={selected} index={i + 1} />
                </span>
              </div>
              <div>
                <div className="text-base font-semibold text-fg">{os.title}</div>
                <div className="mt-0.5 text-xs leading-snug text-fg-muted">{os.blurb}</div>
                <div className="mt-2 text-[11px] text-fg-subtle">{os.requires}</div>
              </div>
            </button>
          );
        })}
      </div>
      {chromeOS && (
        <Note>
          On a Chromebook, the Android app runs out of the box. With the Linux development environment turned on, the Linux
          packages work too.
        </Note>
      )}
      {ios && <Note>There is no iPhone or iPad build. Pick the computer or Android device you will install Corvene on.</Note>}
    </div>
  );
}

function ArchStep(props: { os: OsId; selected: ArchId | null; detected: ArchId | null; onPick: (arch: ArchId) => void }) {
  const { os, selected, detected, onPick } = props;
  const options = ARCH_OPTIONS[os];
  const detectedHere = detected && options.some((o) => o.id === detected) ? detected : null;
  const minorNeeded = options.some((o) => o.minor && (o.id === selected || o.id === detectedHere));
  const [showMinor, setShowMinor] = useState(minorNeeded);
  const visible = options.filter((o) => showMinor || !o.minor);
  const hidden = options.length - visible.length;

  return (
    <div className="space-y-4">
      <div role="radiogroup" aria-labelledby="picker-question" className="space-y-2.5">
        {visible.map((arch, i) => {
          const isSelected = selected === arch.id;
          return (
            <button
              key={arch.id}
              type="button"
              role="radio"
              aria-checked={isSelected}
              onClick={() => onPick(arch.id)}
              className={`group picker-option picker-row-in flex w-full items-start gap-3.5 p-4 text-left ${isSelected ? "is-selected" : ""} ${arch.minor ? "opacity-90" : ""}`}
              style={{ animationDelay: `${i * 35}ms` }}
            >
              <span className="mt-0.5 hidden h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-edge bg-canvas-overlay/70 text-fg-secondary sm:flex">
                <CpuIcon className="h-[18px] w-[18px]" />
              </span>
              <span className="min-w-0 flex-1">
                <span className="flex flex-wrap items-center gap-x-2 gap-y-1">
                  <span className="text-[15px] font-semibold text-fg">{arch.title}</span>
                  {arch.badge && <Badge tone={arch.id === "universal" ? "done" : "success"}>{arch.badge}</Badge>}
                  {detectedHere === arch.id && <Badge tone="accent">This device</Badge>}
                </span>
                <span className="mt-1 block text-[13px] leading-relaxed text-fg-muted">{arch.hint}</span>
                <span className="mt-2.5 flex flex-wrap items-center gap-1.5">
                  <span className="text-[10px] font-semibold uppercase tracking-wider text-fg-subtle">Also called</span>
                  {arch.aka.map((name) => (
                    <span key={name} className="rounded-md border border-edge-muted bg-canvas-inset px-1.5 py-0.5 font-mono text-[10.5px] text-accent-muted">
                      {name}
                    </span>
                  ))}
                </span>
                <span className="mt-1.5 flex flex-wrap items-center gap-1.5">
                  <span className="text-[10px] font-semibold uppercase tracking-wider text-fg-subtle">Found in</span>
                  {arch.examples.map((name) => (
                    <span key={name} className="rounded-full bg-canvas-overlay/80 px-2 py-0.5 text-[10.5px] text-fg-secondary">
                      {name}
                    </span>
                  ))}
                </span>
              </span>
              <SelectMark selected={isSelected} index={i + 1} />
            </button>
          );
        })}
      </div>

      {hidden > 0 && (
        <button type="button" onClick={() => setShowMinor(true)} className="flex items-center gap-1.5 text-xs font-semibold text-accent hover:underline">
          <ChevronDown className="h-3.5 w-3.5" />
          Show {hidden} less common {hidden === 1 ? "option" : "options"} (32-bit and others)
        </button>
      )}

      <details className="group rounded-xl border border-edge bg-canvas-subtle/50 open:bg-canvas-subtle/70">
        <summary className="flex cursor-pointer list-none items-center justify-between gap-3 px-4 py-3 text-sm font-semibold text-fg-secondary hover:text-fg [&::-webkit-details-marker]:hidden">
          <span className="flex items-center gap-2">
            <InfoIcon className="h-4 w-4 text-accent" />
            How do I check?
          </span>
          <ChevronDown className="h-4 w-4 text-fg-muted transition-transform group-open:rotate-180" />
        </summary>
        <div className="space-y-4 border-t border-edge-muted px-4 py-4">
          {ARCH_CHECK[os].map((check) => (
            <div key={check.how} className="space-y-2">
              <p className="text-[13px] leading-relaxed text-fg-muted">{check.how}</p>
              {check.command && (
                <div className="flex items-center justify-between gap-2 rounded-lg border border-edge bg-canvas-inset py-1.5 pl-3 pr-1.5 font-mono text-xs text-accent-muted">
                  <code className="truncate">
                    <span className="select-none text-fg-subtle">$ </span>
                    {check.command}
                  </code>
                  <CopyButton text={check.command} />
                </div>
              )}
              <ul className="divide-y divide-edge-muted overflow-hidden rounded-lg border border-edge-muted">
                {check.readings.map((reading) => (
                  <li key={reading.see}>
                    <button
                      type="button"
                      onClick={() => onPick(reading.pick)}
                      className="flex w-full items-center justify-between gap-3 px-3 py-2 text-left text-xs transition-colors hover:bg-canvas-overlay/60"
                    >
                      <span className="min-w-0 truncate font-mono text-fg-secondary">{reading.see}</span>
                      <span className="flex shrink-0 items-center gap-1 font-semibold text-accent">
                        {archLabel(os, reading.pick)}
                        <ChevronRight className="h-3 w-3" />
                      </span>
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      </details>
    </div>
  );
}

function FormatStep(props: { os: OsId; arch: ArchId; selected: FormatId | null; onPick: (format: FormatId) => void }) {
  const { os, arch, selected, onPick } = props;
  const isBuilt = (format: FormatOption) => !format.archs || format.archs.includes(arch);
  // the packages this processor can't use go last
  const options = [...FORMAT_OPTIONS[os]].sort((a, b) => Number(!isBuilt(a)) - Number(!isBuilt(b)));
  let enabledIndex = 0;
  return (
    <div className="space-y-4">
      <div role="radiogroup" aria-labelledby="picker-question" className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        {options.map((format, i) => {
          const built = isBuilt(format);
          const isSelected = selected === format.id;
          const index = built ? ++enabledIndex : 0;
          return (
            <button
              key={format.id}
              type="button"
              role="radio"
              aria-checked={isSelected}
              disabled={!built}
              onClick={() => onPick(format.id)}
              className={`group picker-option picker-row-in flex flex-col gap-3 p-4 text-left disabled:cursor-not-allowed disabled:opacity-45 sm:[&:last-child:nth-child(odd)]:col-span-2 ${isSelected ? "is-selected" : ""}`}
              style={{ animationDelay: `${i * 35}ms` }}
            >
              <span className="flex w-full items-start justify-between gap-2">
                <span className="flex min-w-0 items-start gap-2.5">
                  {os === "android" ? (
                    <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-edge bg-canvas-overlay/70 text-success">
                      {format.id === "compose" ? <PhoneIcon className="h-[18px] w-[18px]" /> : <TabletIcon className="h-[18px] w-[18px]" />}
                    </span>
                  ) : (
                    <span className="shrink-0 rounded-md border border-edge bg-canvas-inset px-1.5 py-1 font-mono text-[11px] font-semibold text-accent-muted">
                      {format.tag}
                    </span>
                  )}
                  <span className="min-w-0">
                    <span className="block text-[15px] font-semibold leading-tight text-fg">{format.title}</span>
                    {format.badge && built && (
                      <span className="mt-1 inline-block">
                        <Badge tone={format.badge === "Recommended" ? "success" : "accent"}>{format.badge}</Badge>
                      </span>
                    )}
                  </span>
                </span>
                {built && <SelectMark selected={isSelected} index={index} />}
              </span>
              <span className="text-[13px] leading-relaxed text-fg-muted">{format.summary}</span>
              {format.forWhom && (
                <span className="flex items-start gap-1.5 text-xs text-fg-secondary">
                  <TargetIcon className="mt-0.5 h-3.5 w-3.5 shrink-0 text-fg-subtle" />
                  {format.forWhom}
                </span>
              )}
              {built ? (
                <span className="mt-auto space-y-1 border-t border-edge-muted pt-3">
                  {format.good.map((g) => (
                    <span key={g} className="flex items-start gap-1.5 text-xs leading-snug text-fg-secondary">
                      <CheckIcon className="mt-px h-3.5 w-3.5 shrink-0 text-success" />
                      {g}
                    </span>
                  ))}
                  {format.mind.map((m) => (
                    <span key={m} className="flex items-start gap-1.5 text-xs leading-snug text-fg-muted">
                      <span className="mx-[3px] mt-[5px] h-1.5 w-1.5 shrink-0 rounded-full bg-attention" />
                      {m}
                    </span>
                  ))}
                </span>
              ) : (
                <span className="mt-auto border-t border-edge-muted pt-3 text-xs text-fg-muted">
                  Not built for {archLabel(os, arch)}: only x86_64 and ARM64.
                </span>
              )}
            </button>
          );
        })}
      </div>
      {os === "android" && <Note>Can't decide? Install both: they live side by side with separate settings.</Note>}
    </div>
  );
}

function EditionStep(props: {
  answers: Answers;
  version: string;
  assets: ReleaseAsset[];
  onPick: (edition: Edition) => void;
  onPickArm64: () => void;
}) {
  const { answers, version, assets, onPick, onPickArm64 } = props;
  const os = answers.os as OsId;
  const arch = answers.arch as ArchId;
  const format = answers.format as FormatId;
  const standardFile = resolveFile(version, "standard", os, arch, format);
  const fullFile = resolveFile(version, "full", os, arch, format);
  const sizeOf = (file: string | null) => (file ? assets.find((a) => a.name === file)?.size : undefined);
  const standardSize = sizeOf(standardFile);
  const fullSize = sizeOf(fullFile);
  const archCount = os === "macos" && arch === "universal" ? 2 : 1;
  const extra = EDITION_FACTS.fullExtraPerArch * archCount;
  const noUniversalFull = os === "android" && arch === "universal";
  let estimated = false;
  const estimateLine = (edition: Edition) => {
    const mb = estimateMB(edition, os, arch, format);
    if (mb) {
      estimated = true;
      return `≈ ${mb} MB`;
    }
    if (edition === "standard") return "The smaller one";
    if (os === "android") return "Much larger";
    estimated = true;
    return `≈ ${EDITION_FACTS.fullDownloadExtraPerArch * archCount} MB more`;
  };

  const cards: {
    id: Edition;
    title: string;
    badge: string;
    tone: "success" | "done";
    tagline: string;
    size?: number;
    estimate: string;
    installed: string;
    inside: string[];
    bestFor: string;
    mind: string[];
    icon: ReactNode;
  }[] = [
    {
      id: "standard",
      title: "Standard",
      badge: "Most people",
      tone: "success",
      tagline: "Smaller. Grammars download if you ever need them.",
      size: standardSize,
      estimate: estimateLine("standard"),
      installed: os === "macos" ? EDITION_FACTS.standardInstalled : "the smallest install",
      inside: [
        "The whole app: every view, dialog and shortcut",
        "GitHub Desktop's syntax highlighting, built in",
        "Tree-sitter highlighting for 300+ languages is optional: turn it on and the grammars download",
      ],
      bestFor: "Almost everyone. Tree-sitter highlighting is off by default anyway.",
      mind: ["Downloading the grammars needs internet access once"],
      icon: <WifiIcon className="h-5 w-5" />,
    },
    {
      id: "full",
      title: "Full",
      badge: "Works offline",
      tone: "done",
      tagline: "Every grammar inside. Nothing to download, ever.",
      size: fullSize,
      estimate: estimateLine("full"),
      installed: `about ${extra} MB more${archCount === 2 ? " (both architectures)" : ""}`,
      inside: [
        "Everything in Standard",
        "Every tree-sitter grammar built in, for 300+ languages",
        "Turn tree-sitter highlighting on without internet access: fine for air-gapped machines and strict firewalls",
        "Updates to the next Full release",
      ],
      bestFor: "Machines that can't reach GitHub, if you want tree-sitter highlighting on them.",
      mind: [`About ${EDITION_FACTS.fullExtraPerArch} MB more per architecture once installed`, "Cloning and pushing still need your remote"],
      icon: <WifiOffIcon className="h-5 w-5" />,
    },
  ];

  return (
    <div className="space-y-4">
      {noUniversalFull && (
        <Note tone="attention">
          The Full edition has no universal APK: with every grammar for four architectures it would be several hundred MB.{" "}
          <button type="button" onClick={onPickArm64} className="font-semibold text-accent hover:underline">
            Get Full for arm64 instead
          </button>
        </Note>
      )}
      <div role="radiogroup" aria-labelledby="picker-question" className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        {cards.map((card, i) => {
          const disabled = card.id === "full" && noUniversalFull;
          const isSelected = answers.edition === card.id;
          const toneText = card.tone === "done" ? "text-done" : "text-success";
          const toneBg = card.tone === "done" ? "bg-done-emphasis/15 border-done-emphasis/40" : "bg-success-emphasis/15 border-success-emphasis/40";
          return (
            <button
              key={card.id}
              type="button"
              role="radio"
              aria-checked={isSelected}
              disabled={disabled}
              onClick={() => onPick(card.id)}
              data-tone={card.tone}
              className={`group picker-option picker-row-in flex flex-col gap-4 p-5 text-left disabled:cursor-not-allowed disabled:opacity-50 ${isSelected ? "is-selected" : ""}`}
              style={{ animationDelay: `${i * 50}ms` }}
            >
              <span className="flex w-full items-start justify-between gap-2">
                <span className="flex items-center gap-3">
                  <span className={`flex h-10 w-10 items-center justify-center rounded-xl border ${toneBg} ${toneText}`}>{card.icon}</span>
                  <span>
                    <span className="block font-display text-lg font-bold leading-tight text-fg">{card.title}</span>
                    <span className="mt-1 inline-block">
                      <Badge tone={card.tone}>{card.badge}</Badge>
                    </span>
                  </span>
                </span>
                {!disabled && <SelectMark selected={isSelected} index={i + 1} />}
              </span>

              <span className="text-[13px] font-medium text-fg-secondary">{card.tagline}</span>

              <span className="grid grid-cols-2 gap-2 rounded-xl border border-edge-muted bg-canvas-inset/70 p-3">
                <span>
                  <span className="block text-[10px] font-semibold uppercase tracking-wider text-fg-subtle">Download</span>
                  {card.size ? (
                    <span className="mt-0.5 block text-lg font-semibold tabular-nums text-fg">{formatSize(card.size).join(" ")}</span>
                  ) : (
                    <span className={`mt-0.5 block font-semibold tabular-nums ${card.estimate.startsWith("≈") ? "text-lg text-fg" : "text-xs leading-6 text-fg-muted"}`}>
                      {card.estimate}
                    </span>
                  )}
                </span>
                <span>
                  <span className="block text-[10px] font-semibold uppercase tracking-wider text-fg-subtle">Installed</span>
                  <span className={`mt-1 block text-xs leading-snug ${card.id === "full" ? toneText : "text-fg-muted"}`}>{card.installed}</span>
                </span>
              </span>

              <span className="space-y-1.5">
                {card.inside.map((line) => (
                  <span key={line} className="flex items-start gap-1.5 text-xs leading-snug text-fg-secondary">
                    <CheckIcon className={`mt-px h-3.5 w-3.5 shrink-0 ${toneText}`} />
                    {line}
                  </span>
                ))}
                {card.mind.map((line) => (
                  <span key={line} className="flex items-start gap-1.5 text-xs leading-snug text-fg-muted">
                    <span className="mx-[3px] mt-[5px] h-1.5 w-1.5 shrink-0 rounded-full bg-attention" />
                    {line}
                  </span>
                ))}
              </span>

              <span className="mt-auto border-t border-edge-muted pt-3 text-xs leading-relaxed text-fg-muted">
                <span className="font-semibold text-fg-secondary">Best for: </span>
                {card.bestFor}
              </span>
            </button>
          );
        })}
      </div>

      {!standardSize && !fullSize && (
        <p className="text-center text-[11px] text-fg-subtle">
          {estimated ? "≈ sizes are from the last measured release. " : ""}Exact sizes show here once the release carries these files.
        </p>
      )}
    </div>
  );
}

function ResultStep(props: {
  answers: { os: OsId; arch: ArchId; format: FormatId; edition: Edition };
  version: string;
  asset: ReleaseAsset | undefined;
  loaded: boolean;
  onDownload: (href: string) => void;
  onSwitchApp: () => void;
  onShowAll: () => void;
}) {
  const { answers, version, asset, loaded, onDownload, onSwitchApp, onShowAll } = props;
  const { os, arch, format, edition } = answers;
  const full = edition === "full";
  const formatOption = FORMAT_OPTIONS[os].find((f) => f.id === format);
  const file = resolveFile(version, edition, os, arch, format);
  const href = file ? assetUrl(version, file, asset ? [asset] : []) : null;
  const sha = asset?.digest?.startsWith("sha256:") ? asset.digest.slice(7) : "";
  const steps = installSteps(os, format, file);
  const { copied, copy } = useCopy();
  const osName = OS_OPTIONS.find((o) => o.id === os)?.title ?? os;
  const appName = format === "legacy" ? "Corvene Legacy" : "Corvene";
  const estimate = estimateMB(edition, os, arch, format);
  const lightlyTested = ARCH_OPTIONS[os].find((a) => a.id === arch)?.minor;

  return (
    <div className="space-y-6">
      <div className={`picker-ticket ${full ? "is-full" : ""}`}>
        <div className="relative space-y-5 p-5 sm:p-6">
          <div className="flex items-start gap-4">
            <span className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl border border-edge bg-canvas-overlay/80 shadow-inner">
              {osIcon(os, "h-7 w-7")}
            </span>
            <div className="min-w-0">
              <h2 id="picker-question" className="font-display text-xl font-bold leading-tight tracking-tight text-fg sm:text-2xl">
                {appName}
                {full && <span className="text-done"> Full</span>} for {osName}
              </h2>
              <div className="mt-2 flex flex-wrap gap-1.5">
                <Badge tone="neutral">{archLabel(os, arch)}</Badge>
                {formatOption && <Badge tone="neutral">{formatLabel(formatOption)}</Badge>}
                <Badge tone={full ? "done" : "success"}>{full ? "Full · works offline" : "Standard"}</Badge>
              </div>
            </div>
          </div>

          {format === "brew" ? (
            <div className="space-y-2">
              <div className="flex items-center justify-between gap-2 rounded-xl border border-edge bg-canvas-inset py-2 pl-4 pr-2 font-mono text-[13px] text-accent-muted">
                <code className="truncate">
                  <span className="select-none text-fg-subtle">$ </span>
                  {BREW_COMMAND}
                </code>
                <CopyButton text={BREW_COMMAND} />
              </div>
              <p className="text-xs text-fg-muted">
                Installs the {os === "macos" ? "universal app" : `${archLabel(os, arch)} AppImage`}, Standard edition. For Full,
                go back and pick another package.
              </p>
            </div>
          ) : file && href ? (
            <>
              <div className="rounded-xl border border-edge-muted bg-canvas-inset/80 p-3.5">
                <div className="flex items-center justify-between gap-3">
                  <span className="min-w-0 truncate font-mono text-xs text-fg-secondary" title={file}>
                    {file}
                  </span>
                  {asset?.size ? (
                    <span className="shrink-0 font-mono text-xs font-semibold tabular-nums text-fg">{formatSize(asset.size).join(" ")}</span>
                  ) : estimate ? (
                    <span className="shrink-0 font-mono text-xs tabular-nums text-fg-muted" title="Measured on an earlier release">
                      ≈ {estimate} MB
                    </span>
                  ) : null}
                </div>
                {sha && (
                  <div className="mt-2 flex items-center gap-2 font-mono text-[10.5px] text-fg-subtle">
                    <span className="text-success">sha256</span>
                    <span className="min-w-0 truncate">{sha}</span>
                    <button
                      type="button"
                      onClick={() => copy(sha)}
                      className="shrink-0 rounded px-1.5 py-0.5 font-sans font-semibold text-accent hover:bg-canvas-overlay"
                    >
                      {copied ? "Copied" : "Copy"}
                    </button>
                  </div>
                )}
                {loaded && !asset && (
                  <div className="mt-2 flex items-start gap-1.5 text-[11px] leading-relaxed text-fg-muted">
                    <span className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-attention" />
                    <span>
                      Not part of <span className="font-mono text-fg-secondary">v{version}</span> yet: the link works once the
                      next release carries it.
                    </span>
                  </div>
                )}
              </div>
              <div className="flex flex-col gap-2 sm:flex-row">
                <a
                  href={href}
                  onClick={() => onDownload(href)}
                  data-autofocus
                  className={`btn flex-1 border-transparent py-3 text-sm text-white shadow-lg ${
                    full ? "bg-done-emphasis shadow-done-emphasis/30 hover:bg-done" : "bg-success-emphasis shadow-success-emphasis/30 hover:bg-success-hover"
                  }`}
                >
                  <DownloadIcon className="h-4 w-4" />
                  Download{asset?.size ? ` · ${formatSize(asset.size).join(" ")}` : ""}
                </a>
                <CopyLinkButton href={href} />
              </div>
            </>
          ) : null}
        </div>
      </div>

      {lightlyTested && (
        <Note tone="attention">
          {archLabel(os, arch)} builds are made by every release but have had little testing. Reports of what works are
          welcome.
        </Note>
      )}

      <section aria-labelledby="picker-install" className="space-y-3">
        <h3 id="picker-install" className="eyebrow text-fg-muted">
          Then install it
        </h3>
        <ol className="space-y-3">
          {steps.map((s, i) => (
            <li key={i} className="picker-row-in flex gap-3" style={{ animationDelay: `${120 + i * 60}ms` }}>
              <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full border border-edge bg-canvas-overlay text-[11px] font-semibold text-fg-secondary">
                {i + 1}
              </span>
              <div className="min-w-0 flex-1 space-y-2 pt-0.5">
                <p className="text-[13px] leading-relaxed text-fg-secondary">{s.text}</p>
                {s.command && (
                  <div className="flex items-start justify-between gap-2 rounded-lg border border-edge bg-canvas-inset py-1.5 pl-3 pr-1.5 font-mono text-xs text-accent-muted">
                    <pre className="min-w-0 flex-1 whitespace-pre-wrap break-all py-0.5">{s.command}</pre>
                    <CopyButton text={s.command} />
                  </div>
                )}
              </div>
            </li>
          ))}
        </ol>
      </section>

      <div className="flex flex-wrap items-center justify-between gap-3 border-t border-edge-muted pt-4 text-xs">
        {os === "android" ? (
          <button type="button" onClick={onSwitchApp} className="font-semibold text-accent hover:underline">
            Also get {format === "compose" ? "Corvene Legacy" : "the new Corvene app"} →
          </button>
        ) : (
          <span className="text-fg-subtle">Updates keep your repositories, accounts and settings.</span>
        )}
        <button type="button" onClick={onShowAll} className="font-semibold text-fg-muted hover:text-fg hover:underline">
          See every package
        </button>
      </div>
    </div>
  );
}

function CopyLinkButton({ href }: { href: string }) {
  const { copied, copy } = useCopy();
  return (
    <button type="button" onClick={() => copy(href)} className="btn-default py-3 text-sm sm:px-5">
      {copied ? <CheckIcon className="h-4 w-4 text-success" /> : <LinkIcon className="h-4 w-4 text-fg-muted" />}
      {copied ? "Link copied" : "Copy link"}
    </button>
  );
}

/* ---------- small parts ---------- */

function SelectMark({ selected, index }: { selected: boolean; index: number }) {
  return (
    <span className="relative flex h-5 w-5 shrink-0 items-center justify-center">
      <span
        className={`absolute inset-0 rounded-full border transition-all duration-200 ${
          selected ? "scale-100 border-transparent bg-accent-emphasis" : "border-edge bg-canvas-inset group-hover:border-edge-hover"
        }`}
      />
      {selected ? (
        <CheckIcon className="picker-check relative h-3 w-3 text-white" />
      ) : (
        <span className="relative hidden font-mono text-[9px] text-fg-subtle md:block" aria-hidden="true">
          {index}
        </span>
      )}
    </span>
  );
}

function Badge({ tone, children }: { tone: "accent" | "success" | "done" | "attention" | "neutral"; children: ReactNode }) {
  const tones = {
    accent: "border-accent-emphasis/40 bg-accent-emphasis/15 text-accent-muted",
    success: "border-success-emphasis/40 bg-success-emphasis/15 text-success",
    done: "border-done-emphasis/40 bg-done-emphasis/15 text-done-muted",
    attention: "border-attention/40 bg-attention/10 text-attention-bright",
    neutral: "border-edge bg-canvas-overlay/70 text-fg-secondary",
  };
  return (
    <span className={`inline-flex items-center whitespace-nowrap rounded-full border px-2 py-px text-[10.5px] font-semibold ${tones[tone]}`}>{children}</span>
  );
}

function Note({ children, tone = "accent" }: { children: ReactNode; tone?: "accent" | "attention" }) {
  return (
    <div
      className={`flex items-start gap-2.5 rounded-xl border p-3.5 text-[13px] leading-relaxed text-fg-secondary ${
        tone === "attention" ? "border-attention/40 bg-attention/10" : "border-accent-emphasis/30 bg-accent-emphasis/10"
      }`}
    >
      <InfoIcon className={`mt-0.5 h-4 w-4 shrink-0 ${tone === "attention" ? "text-attention" : "text-accent"}`} />
      <div>{children}</div>
    </div>
  );
}

/* ---------- icons ---------- */

type IconProps = { className?: string };

function SparkleIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M7 1.5c.35 3.1 1.9 4.65 5 5-3.1.35-4.65 1.9-5 5-.35-3.1-1.9-4.65-5-5 3.1-.35 4.65-1.9 5-5Z" />
      <path d="M12.75 9.5c.15 1.35.9 2.1 2.25 2.25-1.35.15-2.1.9-2.25 2.25-.15-1.35-.9-2.1-2.25-2.25 1.35-.15 2.1-.9 2.25-2.25Z" opacity=".7" />
    </svg>
  );
}

function ChevronRight({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M6.22 3.22a.75.75 0 0 1 1.06 0l4.25 4.25a.75.75 0 0 1 0 1.06l-4.25 4.25a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042L9.94 8 6.22 4.28a.75.75 0 0 1 0-1.06Z" />
    </svg>
  );
}

function ChevronLeft({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M9.78 12.78a.75.75 0 0 1-1.06 0L4.47 8.53a.75.75 0 0 1 0-1.06l4.25-4.25a.751.751 0 0 1 1.042.018.751.751 0 0 1 .018 1.042L6.06 8l3.72 3.72a.75.75 0 0 1 0 1.06Z" />
    </svg>
  );
}

function ChevronDown({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M12.78 5.22a.749.749 0 0 1 0 1.06l-4.25 4.25a.749.749 0 0 1-1.06 0L3.22 6.28a.749.749 0 1 1 1.06-1.06L8 8.939l3.72-3.719a.749.749 0 0 1 1.06 0Z" />
    </svg>
  );
}

function CloseIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M3.72 3.72a.75.75 0 0 1 1.06 0L8 6.94l3.22-3.22a.749.749 0 0 1 1.275.326.749.749 0 0 1-.215.734L9.06 8l3.22 3.22a.749.749 0 0 1-.326 1.275.749.749 0 0 1-.734-.215L8 9.06l-3.22 3.22a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042L6.94 8 3.72 4.78a.75.75 0 0 1 0-1.06Z" />
    </svg>
  );
}

function CheckIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M13.78 4.22a.75.75 0 0 1 0 1.06l-7.25 7.25a.75.75 0 0 1-1.06 0L2.22 9.28a.751.751 0 0 1 .018-1.042.751.751 0 0 1 1.042-.018L6 10.94l6.72-6.72a.75.75 0 0 1 1.06 0Z" />
    </svg>
  );
}

function DownloadIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M2.75 14A1.75 1.75 0 0 1 1 12.25v-2.5a.75.75 0 0 1 1.5 0v2.5c0 .138.112.25.25.25h10.5a.25.25 0 0 0 .25-.25v-2.5a.75.75 0 0 1 1.5 0v2.5A1.75 1.75 0 0 1 13.25 14ZM7.25 7.689V2a.75.75 0 0 1 1.5 0v5.689l1.97-1.969a.749.749 0 1 1 1.06 1.06l-3.25 3.25a.749.749 0 0 1-1.06 0L4.22 6.78a.749.749 0 1 1 1.06-1.06l1.97 1.969Z" />
    </svg>
  );
}

function InfoIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="M0 8a8 8 0 1 1 16 0A8 8 0 0 1 0 8Zm8-6.5a6.5 6.5 0 1 0 0 13 6.5 6.5 0 0 0 0-13ZM6.5 7.75A.75.75 0 0 1 7.25 7h1a.75.75 0 0 1 .75.75v2.75h.25a.75.75 0 0 1 0 1.5h-2a.75.75 0 0 1 0-1.5h.25v-2h-.25a.75.75 0 0 1-.75-.75ZM8 6a1 1 0 1 1 0-2 1 1 0 0 1 0 2Z" />
    </svg>
  );
}

function LinkIcon({ className }: IconProps) {
  return (
    <svg className={className} viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
      <path d="m7.775 3.275 1.25-1.25a3.5 3.5 0 1 1 4.95 4.95l-2.5 2.5a3.5 3.5 0 0 1-4.95 0 .751.751 0 0 1 .018-1.042.751.751 0 0 1 1.042-.018 1.998 1.998 0 0 0 2.83 0l2.5-2.5a2.002 2.002 0 0 0-2.83-2.83l-1.25 1.25a.751.751 0 0 1-1.042-.018.751.751 0 0 1-.018-1.042Zm-4.69 9.64a1.998 1.998 0 0 0 2.83 0l1.25-1.25a.751.751 0 0 1 1.042.018.751.751 0 0 1 .018 1.042l-1.25 1.25a3.5 3.5 0 1 1-4.95-4.95l2.5-2.5a3.5 3.5 0 0 1 4.95 0 .751.751 0 0 1-.018 1.042.751.751 0 0 1-1.042.018 1.998 1.998 0 0 0-2.83 0l-2.5 2.5a1.998 1.998 0 0 0 0 2.83Z" />
    </svg>
  );
}

function StrokeIcon({ className, children }: IconProps & { children: ReactNode }) {
  return (
    <svg className={className} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      {children}
    </svg>
  );
}

function CpuIcon({ className }: IconProps) {
  return (
    <StrokeIcon className={className}>
      <rect x="5" y="5" width="14" height="14" rx="2.5" />
      <rect x="9" y="9" width="6" height="6" rx="1" />
      <path d="M9 2v3M15 2v3M9 19v3M15 19v3M19 9h3M19 15h3M2 9h3M2 15h3" />
    </StrokeIcon>
  );
}

function PhoneIcon({ className }: IconProps) {
  return (
    <StrokeIcon className={className}>
      <rect x="7" y="2.5" width="10" height="19" rx="2.5" />
      <path d="M11 18.5h2" />
    </StrokeIcon>
  );
}

function TabletIcon({ className }: IconProps) {
  return (
    <StrokeIcon className={className}>
      <rect x="2.5" y="4" width="19" height="13" rx="2" />
      <path d="M8 21h8M12 17v4" />
    </StrokeIcon>
  );
}

function TargetIcon({ className }: IconProps) {
  return (
    <StrokeIcon className={className}>
      <circle cx="12" cy="12" r="9" />
      <circle cx="12" cy="12" r="4" />
    </StrokeIcon>
  );
}

function WifiIcon({ className }: IconProps) {
  return (
    <StrokeIcon className={className}>
      <path d="M5 12.55a11 11 0 0 1 14 0M1.5 9a16 16 0 0 1 21 0M8.5 16.1a6 6 0 0 1 7 0" />
      <path d="M12 20h.01" />
    </StrokeIcon>
  );
}

function WifiOffIcon({ className }: IconProps) {
  return (
    <StrokeIcon className={className}>
      <path d="m2 2 20 20M16.7 11.1a11 11 0 0 1 2.3 1.45M5 12.55a11 11 0 0 1 5.2-2.4M10.7 5.05A16 16 0 0 1 22.5 9M1.5 9a16 16 0 0 1 4.6-2.9M8.5 16.1a6 6 0 0 1 7 0" />
      <path d="M12 20h.01" />
    </StrokeIcon>
  );
}
