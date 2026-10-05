import { useId, useState, type KeyboardEvent } from "react";
import { BENCHMARK_METRICS, SYSTEM_SPECS, formatMetric } from "../data/benchmarks";

/**
 * One metric at a time: Corvene against GitHub Desktop and GitKraken. The
 * tag next to each app is what it is built with. Tabs, no auto-advance (a carousel that moves on
 * its own is hard to read and fails WCAG 2.2.2).
 */
export default function BenchmarkComparison() {
  const [activeId, setActiveId] = useState(BENCHMARK_METRICS[0].id);
  const tabsId = useId();
  const metric = BENCHMARK_METRICS.find((m) => m.id === activeId) ?? BENCHMARK_METRICS[0];

  const values = [metric.corvene, metric.githubDesktop ?? 0, metric.gitKraken ?? 0];
  const max = Math.max(...values);
  const widthOf = (value: number) => `${Math.max(2, Math.round((value / max) * 100))}%`;

  const onKeyDown = (e: KeyboardEvent) => {
    const index = BENCHMARK_METRICS.findIndex((m) => m.id === activeId);
    if (e.key === "ArrowRight" || e.key === "ArrowLeft") {
      e.preventDefault();
      const delta = e.key === "ArrowRight" ? 1 : -1;
      const next = BENCHMARK_METRICS[(index + delta + BENCHMARK_METRICS.length) % BENCHMARK_METRICS.length];
      setActiveId(next.id);
      document.getElementById(`${tabsId}-tab-${next.id}`)?.focus();
    }
  };

  return (
    <div className="card mx-auto w-full max-w-5xl bg-gradient-to-b from-canvas-subtle/90 to-canvas p-6 shadow-2xl sm:p-8">
      <div role="tablist" aria-label="Benchmark" className="mb-8 flex flex-wrap items-center justify-center gap-2" onKeyDown={onKeyDown}>
        {BENCHMARK_METRICS.map((m) => {
          const selected = m.id === activeId;
          return (
            <button
              key={m.id}
              id={`${tabsId}-tab-${m.id}`}
              role="tab"
              type="button"
              aria-selected={selected}
              aria-controls={`${tabsId}-panel`}
              tabIndex={selected ? 0 : -1}
              onClick={() => setActiveId(m.id)}
              className={`rounded-lg px-3.5 py-2 text-xs font-semibold transition-colors sm:text-sm ${
                selected ? "bg-success-emphasis text-white" : "bg-canvas-overlay text-fg-muted hover:bg-canvas-hover hover:text-fg"
              }`}
            >
              {m.name}
            </button>
          );
        })}
      </div>

      <div id={`${tabsId}-panel`} role="tabpanel" aria-labelledby={`${tabsId}-tab-${metric.id}`}>
        <div className="mx-auto mb-8 max-w-xl text-center">
          <h3 className="text-xl font-bold tracking-tight text-fg sm:text-2xl">{metric.name}</h3>
          <p className="mt-2 text-sm text-fg-muted">{metric.description}</p>
        </div>

        <div className="mx-auto max-w-3xl space-y-5">
          <Bar
            label="Corvene"
            tag="GPUI"
            value={formatMetric(metric.corvene, metric.unit)}
            width={widthOf(metric.corvene)}
            barClass="bg-gradient-to-r from-success-emphasis to-success"
            strong
          />
          <Competitor
            label="GitHub Desktop 3.6.6"
            value={metric.githubDesktop}
            unit={metric.unit}
            width={metric.githubDesktop !== null ? widthOf(metric.githubDesktop) : ""}
            barClass="bg-gradient-to-r from-done-emphasis to-done opacity-80"
            dotClass="bg-done/60"
          />
          <Competitor
            label="GitKraken"
            value={metric.gitKraken}
            unavailable={metric.unavailable}
            unit={metric.unit}
            width={metric.gitKraken !== null ? widthOf(metric.gitKraken) : ""}
            barClass="bg-gradient-to-r from-accent-emphasis to-accent opacity-70"
            dotClass="bg-accent/60"
          />
        </div>

        <dl className="mt-8 grid gap-2 border-t border-edge/60 pt-6 text-xs text-fg-muted sm:grid-cols-[auto_1fr] sm:gap-x-4">
          <dt className="font-medium text-fg-secondary">Method</dt>
          <dd>{metric.methodology}</dd>
          <dt className="font-medium text-fg-secondary">Machine</dt>
          <dd>{SYSTEM_SPECS.machine}</dd>
        </dl>
      </div>
    </div>
  );
}

function Competitor(props: {
  label: string;
  value: number | null;
  unavailable?: string;
  unit: "ms" | "MB";
  width: string;
  barClass: string;
  dotClass: string;
}) {
  const { label, value, unavailable, unit, width, barClass, dotClass } = props;
  if (value === null) {
    return (
      <div className="flex items-center justify-between gap-4 rounded-lg border border-dashed border-edge px-4 py-3 text-xs text-fg-muted">
        <span className="flex items-center gap-2">
          <span className={`h-2.5 w-2.5 rounded-full ${dotClass}`} />
          {label}
        </span>
        <span>{unavailable ?? "No equivalent"}</span>
      </div>
    );
  }
  return <Bar label={label} tag="Electron" value={formatMetric(value, unit)} width={width} barClass={barClass} />;
}

function Bar(props: { label: string; tag: string; value: string; width: string; barClass: string; strong?: boolean }) {
  const { label, tag, value, width, barClass, strong } = props;
  return (
    <div className="space-y-1.5">
      <div className="flex items-center justify-between text-xs sm:text-sm">
        <span className={`flex items-center gap-2 ${strong ? "font-bold text-fg" : "font-medium text-fg-muted"}`}>
          <span className={`h-2.5 w-2.5 rounded-full ${barClass}`} />
          {label}
          <span className="rounded border border-edge bg-canvas-overlay px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-wider text-fg-muted">
            {tag}
          </span>
        </span>
        <span className={`font-mono text-sm tabular-nums sm:text-base ${strong ? "font-bold text-success" : "text-fg-secondary"}`}>{value}</span>
      </div>
      <div className="h-3 w-full overflow-hidden rounded-full border border-edge/60 bg-canvas-overlay">
        <div className={`h-full rounded-full transition-[width] duration-500 ease-out ${barClass}`} style={{ width }} />
      </div>
    </div>
  );
}
