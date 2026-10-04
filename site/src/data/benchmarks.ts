/**
 * Performance numbers shown on the site, every one measured.
 *
 * Launch, idle memory and installed size come from `tools/perf/apps.py` on
 * 2026-10-03: Corvene (release build), GitHub Desktop 3.6.6 and GitKraken
 * 12.6.0 launched in turn, five rounds, on the same Apple M2 MacBook Air
 * (16 GB, macOS 26.4) with the 1-minute load average between 9 and 15 (other
 * builds were running). Launch is the time from spawning the executable to its
 * first on-screen window (CGWindowList); the Electron apps' first contentful
 * paint, read over the DevTools protocol, came within 60 ms of the window.
 * Memory is the physical footprint (`footprint`, Activity Monitor's Memory
 * column) of the process and its helpers after 60 s idle with one repository.
 *
 * The interaction latencies were measured the same evening on the `big`
 * fixture repository (tools/perf/fixture.py: 50,000 files, 20,000 commits,
 * 231 changed files, one of them 5,000 changed lines): Corvene over its
 * control socket (tools/perf/bench.py: the input until the frame that draws
 * the requested state), GitHub Desktop and GitKraken over the DevTools
 * protocol (tools/perf/apps_latency.py: real Chromium input, timed inside the
 * renderer from the event's timestamp to the first animation frame whose diff
 * shows the selected file). Medians of five rounds. A `null` means the app
 * has no equivalent action.
 */

export interface BenchmarkMetric {
  id: string;
  name: string;
  /** what the number is and how it was taken */
  description: string;
  methodology: string;
  unit: "ms" | "MB";
  corvene: number;
  /** the project's budget for this metric, where PLAN.md sets one */
  budget?: number;
  /** GitHub Desktop 3.6.6 on the same machine; null until measured */
  githubDesktop: number | null;
  /** GitKraken on the same machine; null until measured */
  gitKraken: number | null;
}

export const BENCHMARK_METRICS: BenchmarkMetric[] = [
  {
    id: "cold-start",
    name: "Launch to window",
    description: "From starting the executable to its first window on screen, with a repository open.",
    methodology:
      "Median of five warm launches each, the three apps in turn (tools/perf/apps.py). Corvene's own `main window opened` span read 248 ms; the Electron apps' first contentful paint followed their window within 60 ms.",
    unit: "ms",
    corvene: 289,
    budget: 300,
    githubDesktop: 2504,
    gitKraken: 4874,
  },
  {
    id: "memory-rss",
    name: "Idle memory",
    description: "Physical memory footprint after a minute idle with one repository open, helper processes included.",
    methodology:
      "`footprint` (Activity Monitor's Memory column) summed over the app and its helpers after 60 s idle: Corvene 1 process (RSS 109 MB), GitHub Desktop 4 (RSS 486 MB), GitKraken 7 (RSS 1,026 MB).",
    unit: "MB",
    corvene: 51,
    budget: 80,
    githubDesktop: 175,
    gitKraken: 575,
  },
  {
    id: "diff-open",
    name: "Open a 5,000-line diff",
    description: "From clicking a changed file with 5,000 changed lines to its diff on screen, in the 50,000-file fixture repository.",
    methodology:
      "Median of five. Corvene: tools/perf/bench.py over CORVENE_CONTROL, click until the frame that draws the diff. GitHub Desktop and GitKraken: tools/perf/apps_latency.py, a DevTools-protocol click timed in the renderer until the first frame whose diff shows the file. GitKraken's Monaco editor only lays out the visible lines, GitHub Desktop renders the whole file.",
    unit: "ms",
    corvene: 6.4,
    budget: 50,
    githubDesktop: 3277,
    gitKraken: 101,
  },
  {
    id: "file-select",
    name: "Select the next file",
    description: "Pressing ↓ in the changes list until the next file's diff is drawn, in the 50,000-file fixture repository.",
    methodology:
      "Median of 25 keypresses (five rounds of five). Corvene over CORVENE_CONTROL, GitHub Desktop over the DevTools protocol, both from the key event to the frame that draws the next diff. GitKraken's file list has no keyboard navigation.",
    unit: "ms",
    corvene: 2.5,
    githubDesktop: 67,
    gitKraken: null,
  },
  {
    id: "binary-size",
    name: "Installed size",
    description: "What the application takes on disk.",
    methodology:
      "`du` on the installed app bundles; Corvene is its 22.7 MB release executable plus the icon (the Full edition adds about 170 MB of grammars).",
    unit: "MB",
    corvene: 23,
    budget: 25,
    githubDesktop: 681,
    gitKraken: 633,
  },
];

/** The 2026-09-30 latency pass: Corvene before and after its optimisation work (.docs/perf.md). */
export const LATENCY_PASS: { action: string; before: string; after: string }[] = [
  { action: "Refresh (focus or watcher), nothing changed", before: "1,300–2,700", after: "140–240" },
  { action: "Select a file", before: "42–145", after: "5–9" },
  { action: "Next file (↓)", before: "35–64", after: "4–5" },
  { action: "Open a 5,000-line diff", before: "12–185", after: "9" },
  { action: "Next commit (↓) in History", before: "56–113", after: "11–12" },
  { action: "Open History", before: "30–63", after: "8–11" },
  { action: "Open the branch list (500 branches)", before: "43–76", after: "7–8" },
  { action: "Branch filter keystroke", before: "13–26", after: "3–4" },
  { action: "Scroll the diff, per frame", before: "9–26", after: "6" },
  { action: "Commit (⌘↩ until the list is empty)", before: "580–2,800", after: "280–460" },
  { action: "Checkout a branch five commits away", before: "540", after: "255" },
  { action: "Switch repository (50,000 files)", before: "356", after: "207" },
];

export const COMPARED = {
  date: "2026-10-03",
  githubDesktop: "GitHub Desktop 3.6.6",
  gitKraken: "GitKraken 12.6.0",
};

export const SYSTEM_SPECS = {
  machine: "Apple M2 MacBook Air, 16 GB, macOS 26.4",
  fixture: "tools/perf/fixture.py big: 50,000 files, 20,000 commits, 500 branches, 231 changes",
};

export function formatMetric(value: number, unit: BenchmarkMetric["unit"]): string {
  return `${value.toLocaleString(undefined, { maximumFractionDigits: 1 })} ${unit}`;
}
