import { Grid } from "@mui/material";
import { SplitButtons, TextInput } from "../core";

type VideoFileExtensionsSettingProps = {
  busy: boolean;
  customExtensions: boolean;
  extChanged: boolean;
  extDraft: string;
  onSaveExtensions: (text: string | null) => void;
  setExtDraft: (value: React.SetStateAction<string>) => void;
};

export const VideoFileExtensionsSetting = ({
  busy,
  customExtensions,
  extChanged,
  extDraft,
  onSaveExtensions,
  setExtDraft,
}: VideoFileExtensionsSettingProps) => {
  return (
    <Grid container>
      <Grid size={12}>
        <TextInput
          ids={{
            helperText: "settings-pannel-extensions-helper-text",
            input: "settings-pannel-extensions-input",
            label: "settings-pannel-extensions-label",
          }}
          formControlProps={{ fullWidth: true }}
          helperText={
            <>
              {customExtensions
                ? "Using your list."
                : "Using the built-in list."}{" "}
              Separate with commas, such as mp4, mkv, ts. Applies to folders you
              add from now on; files you pick by hand are always accepted.
            </>
          }
          inputProps={{ onChange: (e) => setExtDraft(e.target.value) }}
          label="Video File Extensions"
          value={extDraft}
        />
      </Grid>
      <Grid size={12}>
        <SplitButtons
          buttonData={[
            {
              key: "save",
              label: "Save",
              onClick: () => onSaveExtensions(extDraft.trim() || null),
              otherProps: { disabled: busy || !extChanged },
            },
            {
              key: "rest-to-default",
              label: "Reset to Default",
              onClick: () => onSaveExtensions(null),
              otherProps: { disabled: busy || !customExtensions },
            },
          ]}
        />
      </Grid>
    </Grid>
  );
};
