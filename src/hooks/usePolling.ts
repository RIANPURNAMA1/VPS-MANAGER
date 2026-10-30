import { useCallback, useEffect, useRef, useState } from "react";
import { toAppErr } from "../lib/api";

/** Memuat data sekali, lalu opsional berkala. Menyimpan data terakhir saat refresh gagal (untuk indikator stale). */
export function useAsync<T>(fn: () => Promise<T>, deps: unknown[], intervalMs?: number) {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);
  const fnRef = useRef(fn);
  fnRef.current = fn;
  const alive = useRef(true);
  const running = useRef(false);

  const run = useCallback(async () => {
    if (running.current) return;
    running.current = true;
    try {
      const r = await fnRef.current();
      if (!alive.current) return;
      setData(r); setError(null); setUpdatedAt(Date.now());
    } catch (e) {
      if (alive.current) setError(toAppErr(e).message);
    } finally {
      running.current = false;
      if (alive.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    alive.current = true;
    setLoading(true);
    void run();
    const t = intervalMs ? window.setInterval(() => void run(), intervalMs) : undefined;
    return () => { alive.current = false; if (t) window.clearInterval(t); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, intervalMs, run]);

  return { data, error, loading, updatedAt, reload: run, setData };
}
