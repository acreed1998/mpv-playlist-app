import { useEffect, useState } from "react";

interface Props {
  mpvPath: string | null; // the saved value
  busy: boolean;
  onSave: (path: string | null) => void;
  onBrowse: () => Promise<string | null>;
}

export function SettingsPanel({ mpvPath, busy, onSave, onBrowse }: Props) {
  // What's typed in the box, which may differ from what's saved.
  const [draft, setDraft] = useState(mpvPath ?? "");

  // If the saved value changes (after a save), reset the box to match it.
  useEffect(() => setDraft(mpvPath ?? ""), [mpvPath]);

  const changed = draft.trim() !== (mpvPath ?? "");

  return (
    <details style={{ marginBottom: 12 }}>
      <summary>Settings</summary>
      <div style={{ padding: "8px 0" }}>
        <label>
          mpv location{" "}
          <input
            style={{ width: "60%" }}
            placeholder="Leave empty to use mpv from your PATH"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
          />
        </label>{" "}
        <button
          disabled={busy}
          onClick={async () => {
            const chosen = await onBrowse();
            if (chosen) setDraft(chosen);
          }}
        >
          Browse…
        </button>{" "}
        <button
          disabled={busy || !changed}
          onClick={() => onSave(draft.trim() || null)}
        >
          Save
        </button>{" "}
        <button
          disabled={busy || mpvPath === null}
          onClick={() => onSave(null)}
        >
          Use default
        </button>
        <div style={{ opacity: 0.7, fontSize: "0.9em" }}>
          Takes effect the next time you press Play. Use the full path, such as
          /usr/bin/mpv.
        </div>
      </div>
    </details>
  );
}
