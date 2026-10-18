import { formatTime } from "../lib/format";

interface Props {
  mpvReady: boolean;
  busy: boolean;
  trackCount: number;
  resume: { index: number; secs: number } | null;
  onPlay: () => void;
  onResume: () => void;
  onStartOver: () => void;
  onStop: () => void;
}

export function PlaybackControls({
  mpvReady,
  busy,
  trackCount,
  resume,
  onPlay,
  onResume,
  onStartOver,
  onStop,
}: Props) {
  return (
    <div style={{ marginBottom: 12 }}>
      {resume ? (
        <>
          <button onClick={onResume} disabled={!mpvReady || busy}>
            ▶ Resume
          </button>{" "}
          <button onClick={onStartOver} disabled={!mpvReady || busy}>
            ⟲ Start over
          </button>{" "}
        </>
      ) : (
        <button
          onClick={onPlay}
          disabled={!mpvReady || busy || trackCount === 0}
        >
          ▶ Play
        </button>
      )}{" "}
      <button onClick={onStop} disabled={busy}>
        ■ Stop
      </button>
      {resume && (
        <div style={{ opacity: 0.7, fontSize: "0.9em" }}>
          Resume point: track {resume.index + 1} of {trackCount} at{" "}
          {formatTime(resume.secs)}
        </div>
      )}
    </div>
  );
}
