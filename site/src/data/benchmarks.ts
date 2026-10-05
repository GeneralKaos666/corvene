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
  /** what the number is */
  description: string;
  /** how it was taken, one or two sentences */
  methodology: string;
  unit: "ms" | "MB";
  corvene: number;
  /** GitHub Desktop 3.6.6 on the same machine */
  githubDesktop: number | null;
  /** GitKraken 12.6.0 on the same machine; null when it has no such action */
  gitKraken: number | null;
  /** why an app has no number */
  unavailable?: string;
}

export const BENCHMARK_METRICS: BenchmarkMetric[] = [
  {
    id: "cold-start",
    name: "Launch to window",
    description: "Start the app until its window is on screen, with a repository open.",
    methodology: "Median of five launches, the three apps taking turns. The clock stops when the window shows up in CGWindowList.",
    unit: "ms",
    corvene: 289,
    githubDesktop: 2504,
    gitKraken: 4874,
  },
  {
    id: "memory-rss",
    name: "Idle memory",
    description: "Memory after a minute of doing nothing with one repository open, helper processes included.",
    methodology: "Physical footprint, the number Activity Monitor shows, summed over every process of the app.",
    unit: "MB",
    corvene: 51,
    githubDesktop: 175,
    gitKraken: 575,
  },
  {
    id: "diff-open",
    name: "Open a 5,000-line diff",
    description: "Click a file with 5,000 changed lines until its diff is drawn, in a 50,000-file repository.",
    methodology: "Median of five. Each app gets a real click and the clock stops on the first frame that shows the diff.",
    unit: "ms",
    corvene: 6.4,
    githubDesktop: 3277,
    gitKraken: 101,
  },
  {
    id: "file-select",
    name: "Select the next file",
    description: "Press ↓ in the list of changes until the next file's diff is drawn.",
    methodology: "Median of 25 key presses, timed the same way as the click above.",
    unit: "ms",
    corvene: 2.5,
    githubDesktop: 67,
    gitKraken: null,
    unavailable: "No keyboard navigation in its file list",
  },
  {
    id: "binary-size",
    name: "Installed size",
    description: "What the app takes on disk.",
    methodology: "du on the installed app. The Full edition of Corvene adds about 170 MB of grammars.",
    unit: "MB",
    corvene: 23,
    githubDesktop: 681,
    gitKraken: 633,
  },
];

export const COMPARED = {
  date: "2026-10-03",
  githubDesktop: "GitHub Desktop 3.6.6",
  gitKraken: "GitKraken 12.6.0",
};

export const SYSTEM_SPECS = {
  machine: "Apple M2 MacBook Air, 16 GB, macOS 26.4",
  fixture: "50,000 files, 20,000 commits, 500 branches, 231 changed files",
};

export function formatMetric(value: number, unit: BenchmarkMetric["unit"]): string {
  return `${value.toLocaleString(undefined, { maximumFractionDigits: 1 })} ${unit}`;
}
