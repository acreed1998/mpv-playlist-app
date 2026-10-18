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
  useSortable,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { Track } from "../lib/types";
import { fileName } from "../lib/paths";

interface RowProps {
  track: Track;
  index: number;
  isCurrent: boolean;
  isMissing: boolean;
  canPlay: boolean;
  onPlayFrom: (trackId: string) => void;
  onRemove: (trackId: string) => void;
}

function Row({
  track,
  index,
  isCurrent,
  isMissing,
  canPlay,
  onPlayFrom,
  onRemove,
}: RowProps) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: track.id });

  return (
    <li
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
      <span
        {...attributes}
        {...listeners}
        aria-label="Drag to reorder"
        style={{ cursor: "grab", touchAction: "none" }}
      >
        ⠿
      </span>
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
      <button
        onClick={() => onPlayFrom(track.id)}
        disabled={!canPlay || isMissing}
        title="Play from here"
        aria-label="Play from here"
        style={{ marginLeft: "auto" }}
      >
        ▶
      </button>
      <button
        onClick={() => onRemove(track.id)}
        title="Remove from playlist"
        aria-label="Remove from playlist"
      >
        ✕
      </button>
    </li>
  );
}

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
    <div
      style={{ maxHeight: 400, overflow: "auto", border: "1px solid #8884" }}
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
          <ul style={{ listStyle: "none", margin: 0, padding: 0 }}>
            {tracks.map((track, i) => (
              <Row
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
          </ul>
        </SortableContext>
      </DndContext>
    </div>
  );
}
