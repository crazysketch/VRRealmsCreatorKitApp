import { formatBytes, type Progress } from "../lib/api";

export default function ProgressCard({ progress }: { progress: Progress }) {
  const pct = progress.total ? Math.min(100, (progress.done / progress.total) * 100) : null;
  return (
    <div className="card progress-card">
      <div className="progress-label">
        <span>{progress.label}</span>
        <span className="text-dim">
          {progress.total
            ? `${formatBytes(progress.done)} / ${formatBytes(progress.total)}`
            : progress.done > 0
            ? formatBytes(progress.done)
            : ""}
        </span>
      </div>
      <div className="progress-track">
        <div className={`progress-fill ${pct == null ? "unknown" : ""}`} style={{ width: `${pct ?? 0}%` }} />
      </div>
    </div>
  );
}
