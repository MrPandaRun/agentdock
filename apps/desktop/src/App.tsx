import { useEffect, useState } from "react";

import { EmbeddedTerminal } from "@/components/terminal/EmbeddedTerminal";
import { Sidebar } from "@/components/sidebar/Sidebar";
import { ThreadHeader } from "@/components/header/ThreadHeader";
import { Separator } from "@/components/ui/separator";
import { Card, CardContent } from "@/components/ui/card";
import { cn } from "@/lib/utils";

import { useSidebar } from "@/hooks/useSidebar";
import { useThreads } from "@/hooks/useThreads";
import { useWindowDrag } from "@/hooks/useWindowDrag";
import type { AppTheme, TerminalTheme } from "@/types";

const APP_THEME_KEY = "agentdock.desktop.app_theme";

function readStoredAppTheme(): AppTheme {
  if (typeof window === "undefined") {
    return "light";
  }
  const raw = window.localStorage.getItem(APP_THEME_KEY);
  return raw === "dark" || raw === "system" ? raw : "light";
}

function readSystemTheme(): TerminalTheme {
  if (typeof window === "undefined") {
    return "light";
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

function App() {
  const [appTheme, setAppTheme] = useState<AppTheme>(readStoredAppTheme);
  const [systemTheme, setSystemTheme] =
    useState<TerminalTheme>(readSystemTheme);
  const resolvedTheme: TerminalTheme =
    appTheme === "system" ? systemTheme : appTheme;

  const {
    sidebarCollapsed,
    isResizingSidebar,
    layoutGridStyle,
    layoutRef,
    handleSidebarResizeStart,
    toggleSidebar,
  } = useSidebar();

  const {
    selectedThreadId,
    selectedThread,
    folderGroups,
    selectedFolderKey,
    loadingThreads,
    error,
    creatingThreadFolderKey,
    newThreadLaunch,
    newThreadBindingStatus,
    setError,
    loadThreads,
    handleSelectThread,
    handleCreateThreadInFolder,
    handleNewThreadLaunchSettled,
    handleEmbeddedTerminalSessionExit,
  } = useThreads();

  const { dragRegionRef, windowDragStripHeight } = useWindowDrag();

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    window.localStorage.setItem(APP_THEME_KEY, appTheme);
  }, [appTheme]);

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }

    const mediaQuery = window.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = (event: MediaQueryListEvent) => {
      setSystemTheme(event.matches ? "dark" : "light");
    };

    setSystemTheme(mediaQuery.matches ? "dark" : "light");
    mediaQuery.addEventListener("change", handleChange);
    return () => {
      mediaQuery.removeEventListener("change", handleChange);
    };
  }, []);

  useEffect(() => {
    if (typeof document === "undefined") {
      return;
    }
    const root = document.documentElement;
    root.classList.toggle("dark", resolvedTheme === "dark");
    root.style.colorScheme = resolvedTheme;
  }, [resolvedTheme]);

  return (
    <main className="relative h-full min-h-0 overflow-hidden bg-background">
      {/* Drag region for window movement - workaround for Tauri 2.x macOS overlay issue */}
      <div
        ref={dragRegionRef}
        data-window-drag-region="true"
        className="absolute left-0 right-0 top-0 z-[9999] select-none"
        style={{ height: windowDragStripHeight }}
      />
      <section
        ref={layoutRef}
        className="grid h-full min-h-0 flex-1 overflow-hidden"
        style={layoutGridStyle}
      >
        <Sidebar
          sidebarCollapsed={sidebarCollapsed}
          folderGroups={folderGroups}
          selectedFolderKey={selectedFolderKey}
          selectedThreadId={newThreadLaunch ? null : selectedThreadId}
          loadingThreads={loadingThreads}
          creatingThreadFolderKey={creatingThreadFolderKey}
          error={error}
          newThreadBindingStatus={newThreadBindingStatus}
          hasPendingNewThreadLaunch={newThreadLaunch !== null}
          appTheme={appTheme}
          onLoadThreads={loadThreads}
          onSelectThread={handleSelectThread}
          onCreateThread={handleCreateThreadInFolder}
          onAppThemeChange={setAppTheme}
          onClearError={() => setError(null)}
        />

        {!sidebarCollapsed ? (
          <div
            role="separator"
            aria-orientation="vertical"
            className={cn(
              "group flex h-full cursor-col-resize items-center justify-center",
              isResizingSidebar ? "bg-primary/10" : "hover:bg-primary/5",
            )}
            onMouseDown={handleSidebarResizeStart}
          >
            <span
              className={cn(
                "h-14 w-[2px] rounded-full bg-border transition-colors",
                isResizingSidebar
                  ? "bg-primary/55"
                  : "group-hover:bg-primary/45",
              )}
            />
          </div>
        ) : null}

        <Card
          className={cn(
            "flex min-h-0 min-w-0 flex-col rounded-none rounded-tl-xl border-0 bg-card shadow-none",
            sidebarCollapsed ? "col-start-1" : "col-start-3",
          )}
        >
          <ThreadHeader
            sidebarCollapsed={sidebarCollapsed}
            selectedThread={selectedThread}
            newThreadLaunch={newThreadLaunch}
            newThreadBindingStatus={newThreadBindingStatus}
            onToggleSidebar={toggleSidebar}
          />
          <Separator />

          <CardContent className={cn("min-h-0 flex-1", "p-0")}>
            <div className="h-full w-full">
              <EmbeddedTerminal
                thread={
                  newThreadLaunch
                    ? {
                        id: `__new__:${newThreadLaunch.launchId}`,
                        providerId: newThreadLaunch.providerId,
                        projectPath: newThreadLaunch.projectPath,
                      }
                    : selectedThread
                      ? {
                          id: selectedThread.id,
                          providerId: selectedThread.providerId,
                          projectPath: selectedThread.projectPath,
                        }
                      : null
                }
                launchRequest={newThreadLaunch}
                terminalTheme={resolvedTheme}
                onLaunchRequestSettled={handleNewThreadLaunchSettled}
                onActiveSessionExit={handleEmbeddedTerminalSessionExit}
                onError={setError}
              />
            </div>
          </CardContent>
        </Card>
      </section>
    </main>
  );
}

export default App;
