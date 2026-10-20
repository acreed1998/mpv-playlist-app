import { useCallback, useEffect, useMemo, useState } from "react";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { arrayMove } from "@dnd-kit/sortable";
import { Button } from "@mui/material";
import * as api from "./lib/api";
import type {
  DependencyReport,
  Direction,
  Playlist,
  ProbeProgress,
  ResumeState,
  SortKey,
  Track,
  Settings,
} from "./lib/types";
import { describeAdd } from "./lib/format";
import { useMissingFiles } from "./hooks/useMissingFiles";
import { useTauriEvent } from "./hooks/useTauriEvent";
import { DependencyBanner } from "./components/DependencyBanner";
import { PlaybackControls } from "./components/PlaybackControls";
import { SortControls } from "./components/SortControls";
import { TrackList } from "./components/TrackList";
import { SettingsPanel } from "./components/SettingsPanel";

function App() {
  const [deps, setDeps] = useState<DependencyReport | null>(null);
  const [playlist, setPlaylist] = useState<Playlist | null>(null);
  const [extensions, setExtensions] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [probe, setProbe] = useState<ProbeProgress | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [trackingWarning, setTrackingWarning] = useState<string | null>(null);

  useTauriEvent<string | null>("tracking-status", setTrackingWarning);

  const { missingIds, recheck } = useMissingFiles(
    playlist?.tracks.length ?? 0,
    setError,
  );

  // Live updates pushed from Rust.
  useTauriEvent<ResumeState | null>("resume-changed", (resume) =>
    setPlaylist((current) => (current ? { ...current, resume } : current)),
  );
  useTauriEvent<ProbeProgress>("probe-progress", setProbe);

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
    api
      .getSettings()
      .then(setSettings)
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

  async function browseForMpv(): Promise<string | null> {
    const selected = await open({ multiple: false, directory: false });
    return typeof selected === "string" ? selected : null;
  }

  function saveMpvPath(path: string | null) {
    run(async () => {
      setSettings(await api.setMpvPath(path));
      runDependencyCheck(); // refresh the banner and the Play buttons right away
    });
  }

  function saveExtensions(text: string | null) {
    run(async () => {
      setSettings(await api.setVideoExtensions(text));
      setExtensions(await api.getVideoExtensions()); // refresh the file picker's filter
    });
  }

  const savePlaybackSpeed = (speed: string | null) => {
    run(async () => {
      setSettings(await api.setPlaybackSpeed(speed));
    });
  };

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
      if (!proceed) return;
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

  return (
    <main style={{ padding: 24 }}>
      {deps && (
        <DependencyBanner report={deps} onRecheck={runDependencyCheck} />
      )}

      {trackingWarning && (
        <div
          style={{
            border: "1px solid darkorange",
            padding: 12,
            marginBottom: 16,
          }}
        >
          <strong>Progress isn't being tracked.</strong> {trackingWarning}{" "}
          <button onClick={() => setTrackingWarning(null)}>Dismiss</button>
        </div>
      )}

      {settings && (
        <SettingsPanel
          mpvPath={settings.mpvPath}
          playbackSpeed={settings.playbackSpeed}
          extensions={extensions}
          customExtensions={settings.videoExtensions !== null}
          busy={busy}
          onSaveMpvPath={saveMpvPath}
          onBrowse={browseForMpv}
          onSaveExtensions={saveExtensions}
          onSavePlaybackSpeed={savePlaybackSpeed}
        />
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

          <PlaybackControls
            mpvReady={mpvReady}
            busy={busy}
            trackCount={playlist.tracks.length}
            resume={resumeInfo}
            onPlay={() => run(() => api.playPlaylist())}
            onResume={() => run(() => api.playPlaylist(undefined, true))}
            onStartOver={() => run(() => api.playPlaylist(undefined, false))}
            onStop={() => run(() => api.stopVideo())}
          />

          <div style={{ marginBottom: 12 }}>
            <button
              onClick={recheck}
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
          />

          <Button>Mui Works</Button>
        </>
      )}

      {error && <p style={{ color: "crimson" }}>{error}</p>}
    </main>
  );
}

export default App;
