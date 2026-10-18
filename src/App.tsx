import { useCallback, useEffect, useMemo, useState } from "react";
import { open, ask } from "@tauri-apps/plugin-dialog";
import * as api from "./lib/api";
import type {
  AddResult,
  DependencyReport,
  Direction,
  Playlist,
  ResumeState,
  SortKey,
  Track,
  ProbeProgress,
} from "./lib/types";
import { DependencyBanner } from "./components/DependencyBanner";
import { SortControls } from "./components/SortControls";
import { arrayMove } from "@dnd-kit/sortable";
import { TrackList } from "./components/TrackList";
import { listen } from "@tauri-apps/api/event";
import { formatTime } from "./lib/format";

// Turn the backend's counts into a sentence for the user.
function describeAdd(result: AddResult): string {
  const parts = [`Added ${result.added} video${result.added === 1 ? "" : "s"}`];
  if (result.duplicatesSkipped > 0) {
    parts.push(`skipped ${result.duplicatesSkipped} duplicate(s)`);
  }
  if (result.unreadable > 0) {
    parts.push(`${result.unreadable} item(s) could not be read`);
  }
  return parts.join(", ") + ".";
}

function App() {
  const [deps, setDeps] = useState<DependencyReport | null>(null);
  const [playlist, setPlaylist] = useState<Playlist | null>(null);
  const [extensions, setExtensions] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [missingIds, setMissingIds] = useState<Set<string>>(new Set());
  const [checkTick, setCheckTick] = useState(0); // bump this to force a re-check
  const [probe, setProbe] = useState<ProbeProgress | null>(null);

  // Wrap any backend call: reset messages, mark busy, report failures.
  async function run(action: () => Promise<unknown>) {
    setError(null);
    setNotice(null);
    setProbe(null);
    setBusy(true);
    try {
      await action();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
      setProbe(null);
    }
  }
  const runDependencyCheck = useCallback(() => {
    api
      .checkDependencies()
      .then(setDeps)
      .catch((err) => setError(String(err)));
  }, []);

  useEffect(() => {
    runDependencyCheck();
    api
      .loadPlaylist()
      .then(setPlaylist)
      .catch((err) => setError(String(err)));
    api
      .getVideoExtensions()
      .then(setExtensions)
      .catch((err) => setError(String(err)));
  }, [runDependencyCheck]);

  async function addSelection(paths: string[]) {
    const result = await api.addPaths(paths);
    setPlaylist(result.playlist);
    setNotice(describeAdd(result));
  }

  const pickFiles = () =>
    run(async () => {
      const selected = await open({
        multiple: true,
        directory: false,
        filters: [{ name: "Videos", extensions }],
      });
      if (selected) await addSelection(selected);
    });

  const pickFolders = () =>
    run(async () => {
      const selected = await open({ multiple: true, directory: true });
      if (selected) await addSelection(selected);
    });

  async function handleSort(key: SortKey, direction: Direction) {
    if (!playlist) return;

    // Only a hand-arranged order is worth protecting.
    if (playlist.sort.key === "custom") {
      const proceed = await ask(
        "Sorting will replace the order you arranged by hand. This can't be undone.",
        {
          title: "Replace your custom order?",
          kind: "warning",
          okLabel: "Sort anyway",
          cancelLabel: "Keep my order",
        },
      );
      if (!proceed) return; // the user backed out: change nothing
    }

    run(async () => {
      if (key === "length") {
        const result = await api.sortByLength(direction);
        if (result) setPlaylist(result);
        else setNotice("Sort cancelled. Your order is unchanged.");
      } else {
        setPlaylist(await api.sortPlaylist(key, direction));
      }
    });
  }

  function moveTrack(trackId: string, toIndex: number) {
    if (!playlist || busy) return;
    const from = playlist.order.indexOf(trackId);
    if (from < 0 || from === toIndex) return;

    // Show the result immediately, using the same rule Rust uses.
    setPlaylist({
      ...playlist,
      order: arrayMove(playlist.order, from, toIndex),
      sort: { key: "custom", direction: "ascending", seed: null },
    });

    // Then let Rust do the real move and replace our guess with its answer.
    run(async () => {
      try {
        setPlaylist(await api.moveTrack(trackId, toIndex));
      } catch (err) {
        setPlaylist(await api.loadPlaylist()); // undo our guess
        throw err; // let `run` display the error
      }
    });
  }

  function removeOne(trackId: string) {
    if (busy) return;
    run(async () => setPlaylist(await api.removeTracks([trackId])));
  }

  async function removeMissing() {
    const ids = Array.from(missingIds);
    if (ids.length === 0) return;

    const proceed = await ask(
      `Remove ${ids.length} missing file(s) from the playlist? No files are deleted from disk. If a drive is just unplugged, reconnect it and re-check first.`,
      {
        title: "Remove missing files?",
        kind: "warning",
        okLabel: "Remove them",
        cancelLabel: "Keep them",
      },
    );
    if (!proceed) return;

    run(async () => setPlaylist(await api.removeTracks(ids)));
  }

  async function clearAll() {
    if (!playlist || playlist.tracks.length === 0) return;

    const proceed = await ask(
      `Remove all ${playlist.tracks.length} videos from the playlist? This also stops playback and clears your resume point. No files are deleted from disk.`,
      {
        title: "Clear the playlist?",
        kind: "warning",
        okLabel: "Clear playlist",
        cancelLabel: "Keep playlist",
      },
    );
    if (!proceed) return;

    run(async () => setPlaylist(await api.clearPlaylist()));
  }

  // Look up each ID in `order` to get tracks in play order.
  const orderedTracks = useMemo(() => {
    if (!playlist) return [];
    const byId = new Map(playlist.tracks.map((track) => [track.id, track]));
    return playlist.order
      .map((id) => byId.get(id))
      .filter((track): track is Track => track !== undefined);
  }, [playlist]);

  const resumeInfo = useMemo(() => {
    const saved = playlist?.resume;
    if (!saved) return null;
    const index = orderedTracks.findIndex((t) => t.id === saved.trackId);
    if (index < 0) return null;
    return { index, track: orderedTracks[index], secs: saved.positionSecs };
  }, [playlist, orderedTracks]);

  const mpvReady = deps?.mpv.available ?? false;

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<ResumeState | null>("resume-changed", (event) => {
      // The updater form reads the *latest* state, not a stale copy captured earlier.
      setPlaylist((current) =>
        current ? { ...current, resume: event.payload } : current,
      );
    }).then((stop) => {
      if (cancelled)
        stop(); // the component unmounted before the listener was ready
      else unlisten = stop;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const trackCount = playlist?.tracks.length ?? 0;

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
        if (!cancelled) setError(String(err));
      });

    return () => {
      cancelled = true; // a newer check has started; ignore this one's answer
    };
  }, [trackCount, checkTick]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<ProbeProgress>("probe-progress", (event) =>
      setProbe(event.payload),
    ).then((stop) => {
      if (cancelled) stop();
      else unlisten = stop;
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return (
    <main style={{ padding: 24 }}>
      {deps && (
        <DependencyBanner report={deps} onRecheck={runDependencyCheck} />
      )}

      {playlist && (
        <>
          <h2>
            {playlist.name} ({playlist.tracks.length} tracks)
          </h2>
          <div style={{ marginBottom: 12 }}>
            <button
              onClick={pickFiles}
              disabled={busy || extensions.length === 0}
            >
              Add files…
            </button>{" "}
            <button onClick={pickFolders} disabled={busy}>
              Add folders…
            </button>{" "}
            <label>
              <input
                type="checkbox"
                checked={playlist.allowDuplicates}
                disabled={busy}
                onChange={(e) => {
                  const allow = e.target.checked;
                  run(async () =>
                    setPlaylist(await api.setAllowDuplicates(allow)),
                  );
                }}
              />{" "}
              Allow duplicate files
            </label>
          </div>
          <SortControls
            current={playlist.sort}
            busy={busy}
            disabled={playlist.tracks.length === 0}
            ffprobeReady={deps?.ffprobe.available ?? false}
            onSort={handleSort}
          />
          <div style={{ marginBottom: 12 }}>
            {resumeInfo ? (
              <>
                <button
                  onClick={() => run(() => api.playPlaylist(undefined, true))}
                  disabled={!mpvReady || busy}
                >
                  ▶ Resume
                </button>{" "}
                <button
                  onClick={() => run(() => api.playPlaylist(undefined, false))}
                  disabled={!mpvReady || busy}
                >
                  ⟲ Start over
                </button>{" "}
              </>
            ) : (
              <button
                onClick={() => run(() => api.playPlaylist())}
                disabled={!mpvReady || busy || playlist.tracks.length === 0}
              >
                ▶ Play
              </button>
            )}{" "}
            <button onClick={() => run(() => api.stopVideo())} disabled={busy}>
              ■ Stop
            </button>
            {resumeInfo && (
              <div style={{ opacity: 0.7, fontSize: "0.9em" }}>
                Resume point: track {resumeInfo.index + 1} of{" "}
                {orderedTracks.length} at {formatTime(resumeInfo.secs)}
              </div>
            )}
          </div>
          <div style={{ marginBottom: 12 }}>
            <button
              onClick={() => setCheckTick((n) => n + 1)}
              disabled={busy || playlist.tracks.length === 0}
            >
              Re-check files
            </button>{" "}
            <button
              onClick={clearAll}
              disabled={busy || playlist.tracks.length === 0}
            >
              Clear playlist
            </button>
          </div>
          {missingIds.size > 0 && (
            <div
              style={{
                border: "1px solid crimson",
                padding: 8,
                marginBottom: 12,
              }}
            >
              {missingIds.size} file(s) can't be found and will be skipped
              during playback. If a drive is unplugged, reconnect it and
              re-check.{" "}
              <button onClick={removeMissing} disabled={busy}>
                Remove missing
              </button>
            </div>
          )}
          {busy && !(probe && probe.total > 0) && <p>Working…</p>}
          {busy && probe && probe.total > 0 && (
            <p>
              Reading video lengths: {probe.done} of {probe.total}{" "}
              <button onClick={() => void api.cancelProbe()}>Cancel</button>
            </p>
          )}
          {notice && <p>{notice}</p>}
          <TrackList
            tracks={orderedTracks}
            currentTrackId={resumeInfo?.track.id ?? null}
            missingIds={missingIds}
            canPlay={mpvReady && !busy}
            onMove={moveTrack}
            onPlayFrom={(id) => run(() => api.playPlaylist(id))}
            onRemove={removeOne}
          />{" "}
        </>
      )}

      {error && <p style={{ color: "crimson" }}>{error}</p>}
    </main>
  );
}

export default App;
