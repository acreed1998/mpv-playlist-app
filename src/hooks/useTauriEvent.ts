import { useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";

// Subscribe to a named event from Rust for as long as the component is mounted.
export function useTauriEvent<T>(name: string, handler: (payload: T) => void) {
  // Always call the NEWEST handler without re-subscribing when it changes.
  const handlerRef = useRef(handler);
  useEffect(() => {
    handlerRef.current = handler;
  });

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<T>(name, (event) => handlerRef.current(event.payload)).then(
      (stop) => {
        if (cancelled)
          stop(); // unmounted before the listener was ready
        else unlisten = stop;
      },
    );

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [name]);
}
