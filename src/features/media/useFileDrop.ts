import { useRef, useState } from "react";
import type { DragEvent } from "react";

/** Makes an element accept files dragged in from the desktop. (Tauri's own
 * drag-drop handling is disabled in tauri.conf.json so the webview
 * receives these as ordinary HTML drop events.) */
export function useFileDrop(onFiles: (files: File[]) => void) {
  const [dragging, setDragging] = useState(false);
  // dragenter/dragleave fire for every child element crossed; count them
  // so the highlight doesn't flicker.
  const depth = useRef(0);

  const hasFiles = (event: DragEvent) => event.dataTransfer.types.includes("Files");

  return {
    dragging,
    dropProps: {
      onDragEnter: (event: DragEvent) => {
        if (!hasFiles(event)) return;
        event.preventDefault();
        depth.current += 1;
        setDragging(true);
      },
      onDragOver: (event: DragEvent) => {
        if (!hasFiles(event)) return;
        event.preventDefault();
        event.dataTransfer.dropEffect = "copy";
      },
      onDragLeave: (event: DragEvent) => {
        if (!hasFiles(event)) return;
        depth.current = Math.max(0, depth.current - 1);
        if (depth.current === 0) setDragging(false);
      },
      onDrop: (event: DragEvent) => {
        if (!hasFiles(event)) return;
        event.preventDefault();
        depth.current = 0;
        setDragging(false);
        const files = Array.from(event.dataTransfer.files);
        if (files.length > 0) onFiles(files);
      },
    },
  };
}
