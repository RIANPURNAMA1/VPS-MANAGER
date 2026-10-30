/** Grafik garis monokrom: garis hitam/abu, grid tipis, tanpa warna. */
export function LineChart({
  series, height = 90, max, unit = "%", label,
}: {
  series: Array<{ name: string; values: Array<number | null>; dashed?: boolean }>;
  height?: number; max?: number; unit?: string; label: string;
}) {
  const w = 400;
  const all = series.flatMap((s) => s.values.filter((v): v is number => v != null));
  const top = max ?? Math.max(1, ...all) * 1.15;
  const n = Math.max(2, ...series.map((s) => s.values.length));
  const x = (i: number) => (i / (n - 1)) * w;
  const y = (v: number) => height - (Math.min(v, top) / top) * (height - 6) - 3;
  return (
    <figure aria-label={label} className="m-0">
      <svg viewBox={`0 0 ${w} ${height}`} className="w-full" preserveAspectRatio="none" role="img">
        {[0, 0.5, 1].map((g) => (
          <line key={g} x1={0} x2={w} y1={y(top * g)} y2={y(top * g)} stroke="var(--line)" strokeWidth={1} vectorEffect="non-scaling-stroke" />
        ))}
        {series.map((s, si) => {
          const pts = s.values.map((v, i) => (v == null ? null : `${x(i).toFixed(1)},${y(v).toFixed(1)}`)).filter(Boolean);
          if (pts.length < 2) return null;
          return (
            <polyline key={s.name} points={pts.join(" ")} fill="none" stroke={si === 0 ? "var(--fg)" : "var(--fg3)"}
              strokeWidth={1.75} strokeDasharray={s.dashed ? "5 4" : undefined} vectorEffect="non-scaling-stroke" strokeLinejoin="round" />
          );
        })}
      </svg>
      <figcaption className="mt-1 flex justify-between text-xs text-fg2">
        <span>0{unit}</span>
        <span>{series.map((s) => s.name).join(" / ")}</span>
        <span>{top.toFixed(top < 10 ? 1 : 0)}{unit}</span>
      </figcaption>
    </figure>
  );
}

export function Meter({ value, label }: { value: number; label: string }) {
  const v = Math.max(0, Math.min(100, value));
  return (
    <div role="meter" aria-label={label} aria-valuenow={Math.round(v)} aria-valuemin={0} aria-valuemax={100} className="h-1.5 w-full overflow-hidden rounded-md bg-hover">
      <div className="h-full bg-fg" style={{ width: `${v}%` }} />
    </div>
  );
}
