import { Grid } from "@mui/material";
import { SplitButtons, TextInput } from "../core";

type MPVPathSettingProps = {
  busy: boolean;
  mpvChanged: boolean;
  mpvDraft: string;
  mpvPath: string | null;
  onBrowse: () => Promise<string | null>;
  onSaveMpvPath: (path: string | null) => void;
  setMpvDraft: (value: React.SetStateAction<string>) => void;
};

export const MPVPathSetting = ({
  busy,
  mpvChanged,
  mpvDraft,
  mpvPath,
  onBrowse,
  onSaveMpvPath,
  setMpvDraft,
}: MPVPathSettingProps) => {
  return (
    <Grid container sx={{ pb: "1.5rem" }}>
      <Grid size={{ xs: 12 }}>
        <TextInput
          helperText="Leave empty to use mpv from your PATH. Takes effect the next time you press Play. Use the full path, such
            as /usr/bin/mpv."
          ids={{
            helperText: "settings-mpv-path-help-text",
            input: "settings-mpv-path-input",
            label: "settings-mpv-path-labrl",
          }}
          inputProps={{
            defaultValue: mpvDraft,
            onChange: (e) => setMpvDraft(e.target.value),
          }}
          label="mpv location"
          value={mpvDraft}
        />
      </Grid>
      <Grid size={{ xs: 12 }} sx={{ alignContent: "center" }}>
        <SplitButtons
          buttonData={[
            {
              key: "browse",
              label: "Browse",
              onClick: async () => {
                const chosen = await onBrowse();
                if (chosen) setMpvDraft(chosen);
              },
              otherProps: { disabled: busy },
            },
            {
              key: "save",
              label: "Save",
              onClick: () => onSaveMpvPath(mpvDraft.trim() || null),
              otherProps: { disabled: busy || !mpvChanged },
            },
            {
              key: "use-default",
              label: "Use Default",
              onClick: () => onSaveMpvPath(null),
              otherProps: { disabled: busy || mpvPath === null },
            },
          ]}
        />
      </Grid>
    </Grid>
  );
};
