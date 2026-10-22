import { IconButton, ListItem } from "@mui/material";
import PlayArrow from "@mui/icons-material/PlayArrow";
import Close from "@mui/icons-material/Close";
import DragIndicator from "@mui/icons-material/DragIndicator";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { Track } from "../../lib/types";
import { fileName } from "../../lib/paths";

type TrackListRowProps = {
  canPlay: boolean;
  index: number;
  isCurrent: boolean;
  isMissing: boolean;
  key?: string;
  onPlayFrom: (trackId: string) => void;
  onRemove: (trackId: string) => void;
  track: Track;
};

export const TrackListRow = ({
  canPlay,
  index,
  isCurrent,
  isMissing,
  key,
  onPlayFrom,
  onRemove,
  track,
}: TrackListRowProps) => {
  const {
    attributes,
    isDragging,
    listeners,
    setNodeRef,
    transform,
    transition,
  } = useSortable({ id: track.id });

  return (
    <ListItem
      key={key}
      ref={setNodeRef}
      title={isMissing ? `File not found: ${track.path}` : track.path}
      style={{
        transform: CSS.Transform.toString(transform),
        transition,
        opacity: isDragging ? 0.5 : 1,
        display: "flex",
        gap: 8,
        padding: "4px 8px",
        borderBottom: "1px solid #8884",
        background: isCurrent ? "#8883" : undefined,
        fontWeight: isCurrent ? 600 : undefined,
      }}
    >
      <IconButton
        {...attributes}
        {...listeners}
        aria-label="Drag to reorder"
        style={{ cursor: "grab", touchAction: "none" }}
      >
        <DragIndicator />
      </IconButton>
      <span style={{ opacity: 0.6, minWidth: 36 }}>{index + 1}.</span>
      <span
        style={{
          textDecoration: isMissing ? "line-through" : undefined,
          opacity: isMissing ? 0.5 : 1,
        }}
      >
        {fileName(track.path)}
      </span>
      {isMissing && (
        <span style={{ color: "crimson", fontSize: "0.85em" }}>missing</span>
      )}
      <IconButton
        onClick={() => onPlayFrom(track.id)}
        disabled={!canPlay || isMissing}
        title="Play from here"
        aria-label="Play from here"
        style={{ marginLeft: "auto" }}
      >
        <PlayArrow />
      </IconButton>
      <IconButton
        onClick={() => onRemove(track.id)}
        title="Remove from playlist"
        aria-label="Remove from playlist"
      >
        <Close />
      </IconButton>
    </ListItem>
  );
};
