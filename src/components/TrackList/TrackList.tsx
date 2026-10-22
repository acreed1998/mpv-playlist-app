import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import type { Track } from "../../lib/types";
import { Grid, List } from "@mui/material";
import { TrackListRow } from "./TrackListRow";

interface Props {
  tracks: Track[]; // already in play order
  currentTrackId: string | null;
  missingIds: Set<string>;
  canPlay: boolean;
  onMove: (trackId: string, toIndex: number) => void;
  onPlayFrom: (trackId: string) => void;
  onRemove: (trackId: string) => void;
}

export function TrackList({
  tracks,
  currentTrackId,
  missingIds,
  canPlay,
  onMove,
  onPlayFrom,
  onRemove,
}: Props) {
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );

  function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (!over || active.id === over.id) return;

    const to = tracks.findIndex((t) => t.id === over.id);
    if (to >= 0) onMove(String(active.id), to);
  }

  return (
    <Grid
      container
      sx={{ maxHeight: 400, overflow: "auto", border: "1px solid #8884" }}
    >
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        onDragEnd={handleDragEnd}
      >
        <SortableContext
          items={tracks.map((t) => t.id)}
          strategy={verticalListSortingStrategy}
        >
          <List
            style={{
              listStyle: "none",
              margin: 0,
              padding: 0,
              minWidth: "100%",
            }}
          >
            {tracks.map((track, i) => (
              <TrackListRow
                key={track.id}
                track={track}
                index={i}
                isCurrent={track.id === currentTrackId}
                isMissing={missingIds.has(track.id)}
                canPlay={canPlay}
                onPlayFrom={onPlayFrom}
                onRemove={onRemove}
              />
            ))}
          </List>
        </SortableContext>
      </DndContext>
    </Grid>
  );
}
