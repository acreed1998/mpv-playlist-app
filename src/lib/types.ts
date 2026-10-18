export type SortKey =
  | "sourceOrder"
  | "name"
  | "modified"
  | "directory"
  | "length"
  | "random"
  | "shuffleFolders"
  | "custom";

export type Direction = "ascending" | "descending";

export interface Track {
  id: string;
  path: string;
}

export interface ResumeState {
  trackId: string;
  positionSecs: number;
}

export interface Playlist {
  version: number;
  id: string;
  name: string;
  allowDuplicates: boolean;
  tracks: Track[];
  order: string[];
  sort: { key: SortKey; direction: Direction; seed: number | null };
  resume: ResumeState | null;
}

export interface ToolStatus {
  name: string;
  available: boolean;
  version: string | null;
  error: string | null;
}

export interface DependencyReport {
  mpv: ToolStatus;
  ffprobe: ToolStatus;
}

export interface AddResult {
  playlist: Playlist;
  added: number;
  duplicatesSkipped: number;
  unreadable: number;
}

export interface ProbeProgress {
  done: number;
  total: number;
}

export interface Settings {
  mpvPath: string | null;
}
