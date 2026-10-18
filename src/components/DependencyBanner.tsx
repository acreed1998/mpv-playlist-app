import type { DependencyReport, ToolStatus } from "../lib/types";

// What each tool is for, and how to get it. Kept as data so adding a tool later is one line.
const TOOL_INFO: Record<string, { role: string; install: string }> = {
  mpv: {
    role: "mpv is required to play videos.",
    install:
      "Ubuntu/Debian: sudo apt install mpv · macOS: brew install mpv · Windows: scoop install mpv",
  },
  ffprobe: {
    role: "ffprobe is needed only for sorting by video length.",
    install:
      "It comes with FFmpeg. Ubuntu/Debian: sudo apt install ffmpeg · macOS: brew install ffmpeg · Windows: scoop install ffmpeg",
  },
};

interface Props {
  report: DependencyReport;
  onRecheck: () => void;
}

export const DependencyBanner = ({ report, onRecheck }: Props) => {
  const missing: ToolStatus[] = [report.mpv, report.ffprobe].filter(
    (tool) => !tool.available,
  );

  // Everything is installed: show nothing at all.
  if (missing.length === 0) return null;

  return (
    <div style={{ border: "1px solid crimson", padding: 12, marginBottom: 16 }}>
      {missing.map((tool) => (
        <div key={tool.name} style={{ marginBottom: 8 }}>
          <strong>{tool.error}</strong>
          <div>{TOOL_INFO[tool.name]?.role}</div>
          <div style={{ opacity: 0.7 }}>{TOOL_INFO[tool.name]?.install}</div>
        </div>
      ))}
      <button onClick={onRecheck}>Check again</button>
    </div>
  );
};
