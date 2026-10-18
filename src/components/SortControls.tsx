import { useState } from "react";
import type { Direction, Playlist, SortKey } from "../lib/types";

// Record<SortKey, string> means "an entry for EVERY SortKey". If we add a key
// in Rust and mirror it in types.ts, this line fails to compile until we label it.
const LABELS: Record<SortKey, string> = {
  sourceOrder: "Order added",
  name: "Name",
  modified: "Date modified",
  directory: "Folder path",
  length: "Video length",
  random: "Random",
  shuffleFolders: "Shuffle folders, keep order inside",
  custom: "Custom (arranged by hand)",
};

const SELECTABLE: SortKey[] = [
  "name",
  "modified",
  "directory",
  "length",
  "sourceOrder",
  "random",
  "shuffleFolders",
];

// These ignore direction, so the control is disabled for them.
const NO_DIRECTION: SortKey[] = ["random", "shuffleFolders"];

interface Props {
  current: Playlist["sort"];
  busy: boolean;
  disabled: boolean;
  ffprobeReady: boolean;
  onSort: (key: SortKey, direction: Direction) => void;
}

export function SortControls({
  current,
  busy,
  disabled,
  ffprobeReady,
  onSort,
}: Props) {
  const [key, setKey] = useState<SortKey>("name");
  const [direction, setDirection] = useState<Direction>("ascending");

  const ignoresDirection = NO_DIRECTION.includes(key);
  const currentIgnoresDirection =
    NO_DIRECTION.includes(current.key) || current.key === "custom";

  return (
    <div style={{ marginBottom: 12 }}>
      <label>
        Sort by{" "}
        <select value={key} onChange={(e) => setKey(e.target.value as SortKey)}>
          {SELECTABLE.map((k) => {
            const unavailable = k === "length" && !ffprobeReady;
            return (
              <option key={k} value={k} disabled={unavailable}>
                {LABELS[k]}
                {unavailable ? " (needs ffprobe)" : ""}
              </option>
            );
          })}{" "}
        </select>
      </label>{" "}
      <select
        value={direction}
        disabled={ignoresDirection}
        onChange={(e) => setDirection(e.target.value as Direction)}
      >
        <option value="ascending">Ascending (A→Z, oldest first)</option>
        <option value="descending">Descending (Z→A, newest first)</option>
      </select>{" "}
      <button
        onClick={() => onSort(key, direction)}
        disabled={busy || disabled}
      >
        {ignoresDirection ? "Shuffle" : "Sort"}
      </button>
      <div style={{ opacity: 0.7, fontSize: "0.9em" }}>
        Current order: {LABELS[current.key]}
        {!currentIgnoresDirection && ` (${current.direction})`}
      </div>
    </div>
  );
}
