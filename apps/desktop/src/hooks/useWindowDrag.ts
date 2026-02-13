import { useEffect, useRef } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

const WINDOW_DRAG_STRIP_HEIGHT = 32;

export function useWindowDrag(): {
  dragRegionRef: React.RefObject<HTMLDivElement | null>;
  windowDragStripHeight: number;
} {
  const dragRegionRef = useRef<HTMLDivElement | null>(null);
  const appWindow = getCurrentWindow();

  useEffect(() => {
    const dragRegion = dragRegionRef.current;
    if (!dragRegion) {
      return;
    }

    const handleMouseDown = async (event: MouseEvent) => {
      if (event.button !== 0) {
        return;
      }
      void appWindow.startDragging();
    };

    dragRegion.addEventListener("mousedown", handleMouseDown);
    return () => {
      dragRegion.removeEventListener("mousedown", handleMouseDown);
    };
  }, [appWindow]);

  return {
    dragRegionRef,
    windowDragStripHeight: WINDOW_DRAG_STRIP_HEIGHT,
  };
}
