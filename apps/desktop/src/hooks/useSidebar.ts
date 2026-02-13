import type { CSSProperties } from "react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

const SIDEBAR_WIDTH_KEY = "agentdock.desktop.sidebar_width";
const SIDEBAR_COLLAPSED_KEY = "agentdock.desktop.sidebar_collapsed";
const MIN_SIDEBAR_WIDTH = 240;
const MAX_SIDEBAR_WIDTH = 520;

function readStoredSidebarWidth(): number {
  if (typeof window === "undefined") {
    return 300;
  }
  const raw = window.localStorage.getItem(SIDEBAR_WIDTH_KEY);
  const parsed = Number(raw);
  if (!Number.isFinite(parsed)) {
    return 300;
  }
  return Math.min(Math.max(parsed, MIN_SIDEBAR_WIDTH), MAX_SIDEBAR_WIDTH);
}

function readStoredSidebarCollapsed(): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  return window.localStorage.getItem(SIDEBAR_COLLAPSED_KEY) === "1";
}

export interface UseSidebarResult {
  sidebarWidth: number;
  sidebarCollapsed: boolean;
  isResizingSidebar: boolean;
  layoutGridStyle: CSSProperties;
  layoutRef: React.RefObject<HTMLElement | null>;
  handleSidebarResizeStart: (event: React.MouseEvent<HTMLDivElement>) => void;
  toggleSidebar: () => void;
}

export function useSidebar(): UseSidebarResult {
  const [sidebarWidth, setSidebarWidth] = useState<number>(readStoredSidebarWidth);
  const [sidebarCollapsed, setSidebarCollapsed] = useState<boolean>(readStoredSidebarCollapsed);
  const [isResizingSidebar, setIsResizingSidebar] = useState(false);
  const layoutRef = useRef<HTMLElement | null>(null);
  const resizeStateRef = useRef<{ startX: number; startWidth: number } | null>(null);

  const layoutGridStyle = useMemo<CSSProperties>(() => {
    if (sidebarCollapsed) {
      return { gridTemplateColumns: "minmax(0, 1fr)" };
    }
    return {
      gridTemplateColumns: `${sidebarWidth}px 10px minmax(0, 1fr)`,
    };
  }, [sidebarCollapsed, sidebarWidth]);

  const clampSidebarWidth = useCallback((value: number): number => {
    const layoutWidth = layoutRef.current?.clientWidth;
    const maxByContainer = layoutWidth
      ? Math.max(MIN_SIDEBAR_WIDTH, layoutWidth - 420)
      : MAX_SIDEBAR_WIDTH;
    const maxAllowed = Math.min(MAX_SIDEBAR_WIDTH, maxByContainer);
    return Math.min(Math.max(value, MIN_SIDEBAR_WIDTH), maxAllowed);
  }, []);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth));
  }, [sidebarWidth]);

  useEffect(() => {
    const handleResize = () => {
      setSidebarWidth((current) => clampSidebarWidth(current));
    };
    handleResize();
    window.addEventListener("resize", handleResize);
    return () => {
      window.removeEventListener("resize", handleResize);
    };
  }, [clampSidebarWidth]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.setItem(
      SIDEBAR_COLLAPSED_KEY,
      sidebarCollapsed ? "1" : "0",
    );
  }, [sidebarCollapsed]);

  const handleSidebarResizeStart = useCallback(
    (event: React.MouseEvent<HTMLDivElement>) => {
      if (sidebarCollapsed) {
        return;
      }
      resizeStateRef.current = {
        startX: event.clientX,
        startWidth: sidebarWidth,
      };
      setIsResizingSidebar(true);
      event.preventDefault();
    },
    [sidebarCollapsed, sidebarWidth],
  );

  const toggleSidebar = useCallback(() => {
    setSidebarCollapsed((current) => {
      if (current) {
        setSidebarWidth((prev) => clampSidebarWidth(prev));
      }
      return !current;
    });
  }, [clampSidebarWidth]);

  useEffect(() => {
    if (!isResizingSidebar) {
      return;
    }

    const handleMouseMove = (event: MouseEvent) => {
      const resizeState = resizeStateRef.current;
      if (!resizeState) {
        return;
      }
      const deltaX = event.clientX - resizeState.startX;
      const nextWidth = clampSidebarWidth(resizeState.startWidth + deltaX);
      setSidebarWidth(nextWidth);
    };

    const handleMouseUp = () => {
      resizeStateRef.current = null;
      setIsResizingSidebar(false);
    };

    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mouseup", handleMouseUp);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    return () => {
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mouseup", handleMouseUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
  }, [clampSidebarWidth, isResizingSidebar]);

  return {
    sidebarWidth,
    sidebarCollapsed,
    isResizingSidebar,
    layoutGridStyle,
    layoutRef,
    handleSidebarResizeStart,
    toggleSidebar,
  };
}
