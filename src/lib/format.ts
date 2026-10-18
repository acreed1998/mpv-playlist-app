import type { AddResult } from "./types";

export function formatTime(totalSecs: number): string {
  const s = Math.max(0, Math.floor(totalSecs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(sec)}` : `${m}:${pad(sec)}`;
}

// Turn the backend's counts into a sentence for the user.
export function describeAdd(result: AddResult): string {
  const parts = [`Added ${result.added} video${result.added === 1 ? "" : "s"}`];
  if (result.duplicatesSkipped > 0) {
    parts.push(`skipped ${result.duplicatesSkipped} duplicate(s)`);
  }
  if (result.unreadable > 0) {
    parts.push(`${result.unreadable} item(s) could not be read`);
  }
  return parts.join(", ") + ".";
}
