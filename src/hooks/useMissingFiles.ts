import { useEffect, useState } from "react";
import * as api from "../lib/api";

// Tracks which playlist files are missing from disk. Re-checks whenever the
// track count changes, or when `recheck()` is called.
// `onError` should be a stable function (such as a useState setter).
export function useMissingFiles(
  trackCount: number,
  onError: (message: string) => void,
) {
  const [missingIds, setMissingIds] = useState<Set<string>>(new Set());
  const [tick, setTick] = useState(0); // bump this to force a re-check

  useEffect(() => {
    if (trackCount === 0) {
      setMissingIds(new Set());
      return;
    }

    let cancelled = false;
    api
      .findMissingTracks()
      .then((ids) => {
        if (!cancelled) setMissingIds(new Set(ids));
      })
      .catch((err) => {
        if (!cancelled) onError(String(err));
      });

    return () => {
      cancelled = true; // a newer check has started; ignore this one's answer
    };
  }, [trackCount, tick, onError]);

  return { missingIds, recheck: () => setTick((n) => n + 1) };
}
