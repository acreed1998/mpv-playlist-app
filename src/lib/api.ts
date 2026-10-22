import { invoke } from "@tauri-apps/api/core";
import type {
  DependencyReport,
  Playlist,
  AddResult,
  Direction,
  SortKey,
  Settings,
} from "./types";

export const checkDependencies = () =>
  invoke<DependencyReport>("check_dependencies");

export const loadPlaylist = () => invoke<Playlist>("load_playlist");

export const playPlaylist = (startTrackId?: string, resume = false) =>
  invoke<void>("play_playlist", { startTrackId, resume });

export const stopVideo = () => invoke<void>("stop_video");

export const getVideoExtensions = () =>
  invoke<string[]>("get_video_extensions");

export const addPaths = (paths: string[]) =>
  invoke<AddResult>("add_paths", { paths });

export const setAllowDuplicates = (allow: boolean) =>
  invoke<Playlist>("set_allow_duplicates", { allow });

export const sortPlaylist = (key: SortKey, direction: Direction) =>
  invoke<Playlist>("sort_playlist", { key, direction });

export const moveTrack = (trackId: string, toIndex: number) =>
  invoke<Playlist>("move_track", { trackId, toIndex });

export const removeTracks = (ids: string[]) =>
  invoke<Playlist>("remove_tracks", { ids });

export const clearPlaylist = () => invoke<Playlist>("clear_playlist");

export const findMissingTracks = () => invoke<string[]>("find_missing_tracks");

export const sortByLength = (direction: Direction) =>
  invoke<Playlist | null>("sort_by_length", { direction });

export const cancelProbe = () => invoke<void>("cancel_probe");

export const getSettings = () => invoke<Settings>("get_settings");

export const setMpvPath = (path: string | null) =>
  invoke<Settings>("set_mpv_path", { path });

export const setVideoExtensions = (text: string | null) =>
  invoke<Settings>("set_video_extensions", { text });

export const setPlaybackSpeed = (speed: string | null) =>
  invoke<Settings>("set_playback_speed", { speed });

export const setRestartMpvOnTrackEnd = (enabled: boolean) =>
  invoke<Settings>("set_restart_mpv_on_track_end", { enabled });
