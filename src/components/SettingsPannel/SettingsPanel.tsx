import { useEffect, useState } from "react";
import { ExpandCollapse, TextInput } from "../core";
import { Box, Grid } from "@mui/material";
import { MPVPathSetting } from "./MPVPathSetting";
import { VideoFileExtensionsSetting } from "./VideoFileExtensionsSetting";

interface Props {
  mpvPath: string | null; // the saved value
  extensions: string[]; // the list in effect right now (custom or built-in)
  customExtensions: boolean; // true if the user has saved their own list
  busy: boolean;
  playbackSpeed: string | null;
  restartMpvOnTrackEnd: boolean;
  onSaveMpvPath: (path: string | null) => void;
  onBrowse: () => Promise<string | null>;
  onSaveExtensions: (text: string | null) => void;
  onSavePlaybackSpeed: (speed: string | null) => void;
  onSaveRestartMpvOnTrackEnd: (enabled: boolean) => void;
}

export function SettingsPanel({
  mpvPath,
  extensions,
  customExtensions,
  busy,
  playbackSpeed,
  restartMpvOnTrackEnd,
  onSaveMpvPath,
  onBrowse,
  onSaveExtensions,
  onSavePlaybackSpeed,
  onSaveRestartMpvOnTrackEnd,
}: Props) {
  // What's typed in each box, which may differ from what's saved.
  const [mpvDraft, setMpvDraft] = useState(mpvPath ?? "");
  const [extDraft, setExtDraft] = useState("");
  const [speedDraft, setSpeedDraft] = useState("1.00");

  // The list as the box would show it. Used to detect real changes.
  const extText = extensions.join(", ");

  // If a saved value changes (after a save), reset its box to match.
  useEffect(() => {
    setMpvDraft(mpvPath ?? "");
  }, [mpvPath]);
  useEffect(() => {
    setExtDraft(extText);
  }, [extText]);
  useEffect(() => {
    setSpeedDraft(playbackSpeed ?? "1.00");
  }, [playbackSpeed]);

  const mpvChanged = mpvDraft.trim() !== (mpvPath ?? "");
  const extChanged = extDraft.trim() !== extText;

  return (
    <>
      <ExpandCollapse
        headerContent={"Settings"}
        ids={{
          accordion: "settings-expand-collapse",
          accordionDetails: "settings-expand-collapse-content",
          accordionSummary: "settings-expand-collapse-header-button",
        }}
      >
        <Box sx={{ p: "8px 0" }}>
          {/* MPV Path Setting */}
          <MPVPathSetting
            busy={busy}
            mpvChanged={mpvChanged}
            mpvDraft={mpvDraft}
            mpvPath={mpvPath}
            onBrowse={onBrowse}
            onSaveMpvPath={onSaveMpvPath}
            setMpvDraft={setMpvDraft}
          />

          {/* Video File Extensions Setting */}
          <VideoFileExtensionsSetting
            busy={busy}
            customExtensions={customExtensions}
            extChanged={extChanged}
            extDraft={extDraft}
            onSaveExtensions={onSaveExtensions}
            setExtDraft={setExtDraft}
          />

          {/* MPV Playback Speed Setting */}
          <Grid container sx={{ pb: "1.5rem" }}>
            <TextInput
              helperText="Leave empty to use the default playback speed of 1.00"
              ids={{
                helperText: "settings-playback-speed-help-text",
                input: "settings-playback-speed-input",
                label: "settings-playback-speed-labrl",
              }}
              inputProps={{
                defaultValue: speedDraft,
                onChange: (e) => {
                  setSpeedDraft(e.target.value);
                  onSavePlaybackSpeed?.(e.target.value);
                },
              }}
              label="playback speed"
              value={speedDraft}
            />
          </Grid>
          <label>
            <input
              type="checkbox"
              checked={restartMpvOnTrackEnd}
              disabled={busy}
              onChange={(e) => onSaveRestartMpvOnTrackEnd(e.target.checked)}
            />{" "}
            Close and reopen MPV after each video
          </label>
        </Box>
      </ExpandCollapse>
    </>
  );
}
