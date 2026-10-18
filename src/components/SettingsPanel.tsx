import { useEffect, useState } from "react";

interface Props {
  mpvPath: string | null; // the saved value
  extensions: string[]; // the list in effect right now (custom or built-in)
  customExtensions: boolean; // true if the user has saved their own list
  busy: boolean;
  onSaveMpvPath: (path: string | null) => void;
  onBrowse: () => Promise<string | null>;
  onSaveExtensions: (text: string | null) => void;
}

export function SettingsPanel({
  mpvPath,
  extensions,
  customExtensions,
  busy,
  onSaveMpvPath,
  onBrowse,
  onSaveExtensions,
}: Props) {
  // What's typed in each box, which may differ from what's saved.
  const [mpvDraft, setMpvDraft] = useState(mpvPath ?? "");
  const [extDraft, setExtDraft] = useState("");

  // The list as the box would show it. Used to detect real changes.
  const extText = extensions.join(", ");

  // If a saved value changes (after a save), reset its box to match.
  useEffect(() => {
    setMpvDraft(mpvPath ?? "");
  }, [mpvPath]);
  useEffect(() => {
    setExtDraft(extText);
  }, [extText]);

  const mpvChanged = mpvDraft.trim() !== (mpvPath ?? "");
  const extChanged = extDraft.trim() !== extText;

  return (
    <details style={{ marginBottom: 12 }}>
      <summary>Settings</summary>
      <div style={{ padding: "8px 0" }}>
        <label>
          mpv location{" "}
          <input
            style={{ width: "60%" }}
            placeholder="Leave empty to use mpv from your PATH"
            value={mpvDraft}
            onChange={(e) => setMpvDraft(e.target.value)}
          />
        </label>{" "}
        <button
          disabled={busy}
          onClick={async () => {
            const chosen = await onBrowse();
            if (chosen) setMpvDraft(chosen);
          }}
        >
          Browse…
        </button>{" "}
        <button
          disabled={busy || !mpvChanged}
          onClick={() => onSaveMpvPath(mpvDraft.trim() || null)}
        >
          Save
        </button>{" "}
        <button
          disabled={busy || mpvPath === null}
          onClick={() => onSaveMpvPath(null)}
        >
          Use default
        </button>
        <div style={{ opacity: 0.7, fontSize: "0.9em" }}>
          Takes effect the next time you press Play. Use the full path, such as
          /usr/bin/mpv.
        </div>
        <div style={{ marginTop: 16 }}>
          <label>
            Video file extensions{" "}
            <input
              style={{ width: "60%" }}
              value={extDraft}
              onChange={(e) => setExtDraft(e.target.value)}
            />
          </label>{" "}
          <button
            disabled={busy || !extChanged}
            onClick={() => onSaveExtensions(extDraft.trim() || null)}
          >
            Save
          </button>{" "}
          <button
            disabled={busy || !customExtensions}
            onClick={() => onSaveExtensions(null)}
          >
            Reset to default
          </button>
          <div style={{ opacity: 0.7, fontSize: "0.9em" }}>
            {customExtensions ? "Using your list." : "Using the built-in list."}{" "}
            Separate with commas, such as mp4, mkv, ts. Applies to folders you
            add from now on; files you pick by hand are always accepted.
          </div>
        </div>
      </div>
    </details>
  );
}
