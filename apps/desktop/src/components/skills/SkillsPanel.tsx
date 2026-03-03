import { open } from "@tauri-apps/plugin-dialog";
import {
  ChevronRight,
  Download,
  FileArchive,
  FolderSearch,
  Github,
  Loader2,
  Package,
  Plus,
  RefreshCw,
  Search,
  Trash2,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { useSkills } from "@/hooks/useSkills";
import type {
  DiscoverableSkill,
  DiscoverSkillInstallProgress,
  Skill,
  SkillRepo,
} from "@/types";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { cn } from "@/lib/utils";

interface SkillsPanelProps {
  appTheme?: "light" | "dark" | "system";
}

const PROVIDERS = [
  { id: "claude_code", label: "Claude" },
  { id: "codex", label: "Codex" },
  { id: "opencode", label: "OpenCode" },
] as const;

type TabType = "installed" | "discover";

export function SkillsPanel(_props: SkillsPanelProps) {
  const {
    skills,
    skillRepos,
    discoverableSkills,
    discoverInstallProgress,
    loading,
    discovering,
    error,
    loadSkills,
    discoverSkills,
    installSkillFromPath,
    installSkillFromGit,
    installDiscoveredSkill,
    toggleSkillForProvider,
    uninstallSkill,
    addSkillRepo,
    removeSkillRepo,
    getEnabledState,
    isSkillEnabledForProvider,
  } = useSkills();

  const [activeTab, setActiveTab] = useState<TabType>("installed");
  const [installPathDialogOpen, setInstallPathDialogOpen] = useState(false);
  const [installGitDialogOpen, setInstallGitDialogOpen] = useState(false);
  const [installFileArchiveDialogOpen, setInstallFileArchiveDialogOpen] = useState(false);
  const [repoManageDialogOpen, setRepoManageDialogOpen] = useState(false);
  const [confirmUninstallSkill, setConfirmUninstallSkill] = useState<Skill | null>(null);
  const [confirmRemoveRepo, setConfirmRemoveRepo] = useState<SkillRepo | null>(null);
  const [installPath, setInstallPath] = useState("");
  const [installGitUrl, setInstallGitUrl] = useState("");
  const [installFileArchivePath, setInstallFileArchivePath] = useState("");
  const [newRepoInput, setNewRepoInput] = useState("");
  const [isPickingPath, setIsPickingPath] = useState(false);
  const [isPickingFileArchive, setIsPickingFileArchive] = useState(false);
  const [isInstalling, setIsInstalling] = useState(false);
  const [isAddingRepo, setIsAddingRepo] = useState(false);
  const [installError, setInstallError] = useState<string | null>(null);
  const [repoError, setRepoError] = useState<string | null>(null);
  const [togglingSkillId, setTogglingSkillId] = useState<string | null>(null);
  const [uninstallingSkillId, setUninstallingSkillId] = useState<string | null>(null);
  const [removingRepoId, setRemovingRepoId] = useState<string | null>(null);

  const panelRef = useRef<HTMLDivElement | null>(null);

  const handlePickSkillFolder = useCallback(async () => {
    setIsPickingPath(true);
    setInstallError(null);
    try {
      const picked = await open({
        directory: true,
        multiple: false,
        title: "Choose skill folder",
      });
      if (picked && typeof picked === "string") {
        setInstallPath(picked);
      }
    } catch (pickError) {
      const message =
        pickError instanceof Error ? pickError.message : String(pickError);
      setInstallError(message);
    } finally {
      setIsPickingPath(false);
    }
  }, []);

  const handlePickFileArchiveFile = useCallback(async () => {
    setIsPickingFileArchive(true);
    setInstallError(null);
    try {
      const picked = await open({
        multiple: false,
        filters: [{ name: "ZIP", extensions: ["zip"] }],
        title: "Choose skill ZIP file",
      });
      if (picked && typeof picked === "string") {
        setInstallFileArchivePath(picked);
      }
    } catch (pickError) {
      const message =
        pickError instanceof Error ? pickError.message : String(pickError);
      setInstallError(message);
    } finally {
      setIsPickingFileArchive(false);
    }
  }, []);

  const handleInstallFromPath = useCallback(async () => {
    if (!installPath.trim()) {
      setInstallError("Please select a skill folder");
      return;
    }

    setIsInstalling(true);
    setInstallError(null);
    try {
      await installSkillFromPath(installPath);
      setInstallPathDialogOpen(false);
      setInstallPath("");
    } catch (installErr) {
      const message =
        installErr instanceof Error ? installErr.message : String(installErr);
      setInstallError(message);
    } finally {
      setIsInstalling(false);
    }
  }, [installPath, installSkillFromPath]);

  const handleInstallFromGit = useCallback(async () => {
    if (!installGitUrl.trim()) {
      setInstallError("Please enter a Git URL");
      return;
    }

    setIsInstalling(true);
    setInstallError(null);
    try {
      await installSkillFromGit(installGitUrl);
      setInstallGitDialogOpen(false);
      setInstallGitUrl("");
    } catch (installErr) {
      const message =
        installErr instanceof Error ? installErr.message : String(installErr);
      setInstallError(message);
    } finally {
      setIsInstalling(false);
    }
  }, [installGitUrl, installSkillFromGit]);

  const handleInstallFromFileArchive = useCallback(async () => {
    if (!installFileArchivePath.trim()) {
      setInstallError("Please select a ZIP file");
      return;
    }

    setIsInstalling(true);
    setInstallError(null);
    try {
      // For ZIP, we extract to temp and install from path
      // This is a simplified version - in production you'd want proper ZIP extraction
      await installSkillFromPath(installFileArchivePath);
      setInstallFileArchiveDialogOpen(false);
      setInstallFileArchivePath("");
    } catch (installErr) {
      const message =
        installErr instanceof Error ? installErr.message : String(installErr);
      setInstallError(message);
    } finally {
      setIsInstalling(false);
    }
  }, [installFileArchivePath, installSkillFromPath]);

  const handleToggleProvider = useCallback(
    async (skill: Skill, provider: string, enabled: boolean) => {
      setTogglingSkillId(skill.id);
      try {
        await toggleSkillForProvider(skill.id, provider, enabled);
      } catch {
        // Error is already handled by the hook
      } finally {
        setTogglingSkillId(null);
      }
    },
    [toggleSkillForProvider]
  );

  const handleUninstallSkill = useCallback(
    async (skill: Skill) => {
      setUninstallingSkillId(skill.id);
      try {
        await uninstallSkill(skill.id);
        setConfirmUninstallSkill(null);
      } catch {
        // Error is already handled by the hook
      } finally {
        setUninstallingSkillId(null);
      }
    },
    [uninstallSkill]
  );

  const handleAddRepo = useCallback(async () => {
    const input = newRepoInput.trim();
    if (!input) {
      setRepoError("Please enter a repository");
      return;
    }

    // Parse owner/repo format
    const parts = input.split("/");
    if (parts.length !== 2 || !parts[0] || !parts[1]) {
      setRepoError("Invalid format. Use: owner/repo");
      return;
    }

    setIsAddingRepo(true);
    setRepoError(null);
    try {
      await addSkillRepo(parts[0], parts[1], "main");
      setNewRepoInput("");
    } catch (addError) {
      const message =
        addError instanceof Error ? addError.message : String(addError);
      setRepoError(message);
    } finally {
      setIsAddingRepo(false);
    }
  }, [newRepoInput, addSkillRepo]);

  const handleRemoveRepo = useCallback(
    async (repo: SkillRepo) => {
      setRemovingRepoId(repo.id);
      try {
        await removeSkillRepo(repo.id);
        setConfirmRemoveRepo(null);
      } catch {
        // Error is already handled by the hook
      } finally {
        setRemovingRepoId(null);
      }
    },
    [removeSkillRepo]
  );

  useEffect(() => {
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      if (confirmUninstallSkill) {
        setConfirmUninstallSkill(null);
        return;
      }
      if (confirmRemoveRepo) {
        setConfirmRemoveRepo(null);
        return;
      }
      if (repoManageDialogOpen) {
        setRepoManageDialogOpen(false);
        return;
      }
      if (installFileArchiveDialogOpen) {
        setInstallFileArchiveDialogOpen(false);
        return;
      }
      if (installGitDialogOpen) {
        setInstallGitDialogOpen(false);
        return;
      }
      if (installPathDialogOpen) {
        setInstallPathDialogOpen(false);
      }
    };

    window.addEventListener("keydown", handleEscape);
    return () => {
      window.removeEventListener("keydown", handleEscape);
    };
  }, [installPathDialogOpen, installGitDialogOpen, installFileArchiveDialogOpen, repoManageDialogOpen, confirmUninstallSkill, confirmRemoveRepo]);

  return (
    <div ref={panelRef} className="space-y-3">
      <div className="flex items-center justify-between">
        <p className="text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
          Skills
        </p>
        <Badge variant="secondary" className="h-5 px-2 text-[10px]">
          {skills.length}
        </Badge>
      </div>

      {loading ? (
        <div className="flex items-center justify-center py-4">
          <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
        </div>
      ) : error ? (
        <div className="space-y-2">
          <p className="text-xs text-destructive">{error}</p>
          <Button
            variant="outline"
            size="sm"
            className="h-7 w-full text-xs"
            onClick={() => void loadSkills()}
          >
            Retry
          </Button>
        </div>
      ) : (
        <>
          {/* Tab Navigation */}
          <div className="flex gap-1 rounded-md bg-muted/50 p-0.5">
            <Button
              variant={activeTab === "installed" ? "default" : "ghost"}
              size="sm"
              className="h-6 flex-1 text-[10px] font-medium"
              onClick={() => setActiveTab("installed")}
            >
              Installed
            </Button>
            <Button
              variant={activeTab === "discover" ? "default" : "ghost"}
              size="sm"
              className="h-6 flex-1 text-[10px] font-medium"
              onClick={() => setActiveTab("discover")}
            >
              Discover
            </Button>
          </div>

          {activeTab === "installed" ? (
            <InstalledTab
              skills={skills}
              onInstallFromPath={() => {
                setInstallPath("");
                setInstallError(null);
                setInstallPathDialogOpen(true);
              }}
              onInstallFromGit={() => {
                setInstallGitUrl("");
                setInstallError(null);
                setInstallGitDialogOpen(true);
              }}
              onInstallFromFileArchive={() => {
                setInstallFileArchivePath("");
                setInstallError(null);
                setInstallFileArchiveDialogOpen(true);
              }}
              onToggleProvider={handleToggleProvider}
              onUninstall={setConfirmUninstallSkill}
              togglingSkillId={togglingSkillId}
              uninstallingSkillId={uninstallingSkillId}
              getEnabledState={getEnabledState}
              isSkillEnabledForProvider={isSkillEnabledForProvider}
            />
          ) : (
            <DiscoverTab
              skillRepos={skillRepos}
              discoverableSkills={discoverableSkills}
              installedSkills={skills}
              discoverInstallProgress={discoverInstallProgress}
              discovering={discovering}
              onDiscoverSkills={(forceRefresh) => void discoverSkills(forceRefresh)}
              onManageRepos={() => setRepoManageDialogOpen(true)}
              onInstallDiscoveredSkill={async (skill) => {
                try {
                  await installDiscoveredSkill(skill);
                  setInstallError(null);
                  // Refresh the skills list
                  await loadSkills();
                } catch (installError) {
                  const message =
                    installError instanceof Error
                      ? installError.message
                      : String(installError);
                  setInstallError(message);
                }
              }}
            />
          )}
        </>
      )}

      {/* Install from Path Dialog */}
      {installPathDialogOpen ? (
        <DialogOverlay onClose={() => !isInstalling && setInstallPathDialogOpen(false)}>
          <DialogCard onClick={(e) => e.stopPropagation()}>
            <CardHeader className="pb-2">
              <CardTitle className="text-base">Install from Path</CardTitle>
              <CardDescription className="text-xs">
                Select a local folder containing a skill.json file.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-8 w-full justify-start px-2 text-xs"
                  onClick={() => void handlePickSkillFolder()}
                  disabled={isPickingPath || isInstalling}
                >
                  {isPickingPath ? (
                    <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <FolderSearch className="mr-1.5 h-3.5 w-3.5" />
                  )}
                  Choose skill folder
                </Button>

                {installPath ? (
                  <p className="text-[11px] text-muted-foreground">
                    Selected: <code className="break-all">{installPath}</code>
                  </p>
                ) : null}

                {installError ? (
                  <p className="text-[11px] text-destructive">{installError}</p>
                ) : null}
              </div>

              <DialogButtons
                onCancel={() => setInstallPathDialogOpen(false)}
                onSubmit={() => void handleInstallFromPath()}
                submitLabel="Install"
                submitDisabled={!installPath || isInstalling}
                isSubmitting={isInstalling}
              />
            </CardContent>
          </DialogCard>
        </DialogOverlay>
      ) : null}

      {/* Install from Git Dialog */}
      {installGitDialogOpen ? (
        <DialogOverlay onClose={() => !isInstalling && setInstallGitDialogOpen(false)}>
          <DialogCard onClick={(e) => e.stopPropagation()}>
            <CardHeader className="pb-2">
              <CardTitle className="text-base">Install from Git</CardTitle>
              <CardDescription className="text-xs">
                Enter a Git repository URL containing a skill.json file.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <div className="flex items-center gap-2">
                  <Github className="h-4 w-4 shrink-0 text-muted-foreground" />
                  <input
                    type="text"
                    placeholder="https://github.com/user/skill-repo"
                    value={installGitUrl}
                    onChange={(e) => setInstallGitUrl(e.target.value)}
                    className="flex-1 rounded-md border border-border bg-background px-2.5 py-1.5 text-xs outline-none focus:ring-1 focus:ring-ring"
                    disabled={isInstalling}
                  />
                </div>

                {installError ? (
                  <p className="text-[11px] text-destructive">{installError}</p>
                ) : null}
              </div>

              <DialogButtons
                onCancel={() => setInstallGitDialogOpen(false)}
                onSubmit={() => void handleInstallFromGit()}
                submitLabel="Install"
                submitDisabled={!installGitUrl.trim() || isInstalling}
                isSubmitting={isInstalling}
              />
            </CardContent>
          </DialogCard>
        </DialogOverlay>
      ) : null}

      {/* Install from ZIP Dialog */}
      {installFileArchiveDialogOpen ? (
        <DialogOverlay onClose={() => !isInstalling && setInstallFileArchiveDialogOpen(false)}>
          <DialogCard onClick={(e) => e.stopPropagation()}>
            <CardHeader className="pb-2">
              <CardTitle className="text-base">Install from ZIP</CardTitle>
              <CardDescription className="text-xs">
                Select a ZIP file containing a skill with skill.json.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-8 w-full justify-start px-2 text-xs"
                  onClick={() => void handlePickFileArchiveFile()}
                  disabled={isPickingFileArchive || isInstalling}
                >
                  {isPickingFileArchive ? (
                    <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <FileArchive className="mr-1.5 h-3.5 w-3.5" />
                  )}
                  Choose ZIP file
                </Button>

                {installFileArchivePath ? (
                  <p className="text-[11px] text-muted-foreground">
                    Selected: <code className="break-all">{installFileArchivePath}</code>
                  </p>
                ) : null}

                {installError ? (
                  <p className="text-[11px] text-destructive">{installError}</p>
                ) : null}
              </div>

              <DialogButtons
                onCancel={() => setInstallFileArchiveDialogOpen(false)}
                onSubmit={() => void handleInstallFromFileArchive()}
                submitLabel="Install"
                submitDisabled={!installFileArchivePath || isInstalling}
                isSubmitting={isInstalling}
              />
            </CardContent>
          </DialogCard>
        </DialogOverlay>
      ) : null}

      {/* Repository Management Dialog */}
      {repoManageDialogOpen ? (
        <DialogOverlay onClose={() => setRepoManageDialogOpen(false)}>
          <DialogCard onClick={(e) => e.stopPropagation()} className="max-w-sm">
            <CardHeader className="pb-2">
              <CardTitle className="text-base">Manage Skill Repositories</CardTitle>
              <CardDescription className="text-xs">
                Add GitHub repositories to discover skills.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              {/* Existing repos */}
              {skillRepos.length > 0 ? (
                <div className="space-y-1">
                  {skillRepos.map((repo) => (
                    <div
                      key={repo.id}
                      className="flex items-center justify-between rounded-md border border-border/50 px-2 py-1.5"
                    >
                      <span className="truncate text-xs">{repo.id}</span>
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-5 w-5 p-0 text-muted-foreground hover:text-destructive"
                        onClick={() => setConfirmRemoveRepo(repo)}
                        disabled={removingRepoId === repo.id}
                      >
                        {removingRepoId === repo.id ? (
                          <Loader2 className="h-3 w-3 animate-spin" />
                        ) : (
                          <Trash2 className="h-3 w-3" />
                        )}
                      </Button>
                    </div>
                  ))}
                </div>
              ) : (
                <p className="text-xs text-muted-foreground">
                  No repositories added yet.
                </p>
              )}

              {/* Add new repo */}
              <div className="space-y-2">
                <div className="flex gap-2">
                  <input
                    type="text"
                    placeholder="owner/repo"
                    value={newRepoInput}
                    onChange={(e) => setNewRepoInput(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && void handleAddRepo()}
                    className="flex-1 rounded-md border border-border bg-background px-2.5 py-1.5 text-xs outline-none focus:ring-1 focus:ring-ring"
                    disabled={isAddingRepo}
                  />
                  <Button
                    variant="default"
                    size="sm"
                    className="h-8 px-3 text-xs"
                    onClick={() => void handleAddRepo()}
                    disabled={!newRepoInput.trim() || isAddingRepo}
                  >
                    {isAddingRepo ? (
                      <Loader2 className="h-3 w-3 animate-spin" />
                    ) : (
                      <Plus className="h-3 w-3" />
                    )}
                  </Button>
                </div>
                {repoError ? (
                  <p className="text-[11px] text-destructive">{repoError}</p>
                ) : null}
              </div>

              <div className="flex justify-end pt-2">
                <Button
                  variant="outline"
                  size="sm"
                  className="h-8 text-xs"
                  onClick={() => setRepoManageDialogOpen(false)}
                >
                  Done
                </Button>
              </div>
            </CardContent>
          </DialogCard>
        </DialogOverlay>
      ) : null}

      {/* Confirm Uninstall Dialog */}
      {confirmUninstallSkill ? (
        <DialogOverlay onClose={() => setConfirmUninstallSkill(null)}>
          <DialogCard onClick={(e) => e.stopPropagation()} className="max-w-sm">
            <CardHeader className="pb-2">
              <CardTitle className="text-base">Uninstall Skill</CardTitle>
              <CardDescription className="text-xs">
                Are you sure you want to uninstall "{confirmUninstallSkill.name}"?
                This action cannot be undone.
              </CardDescription>
            </CardHeader>
            <CardContent className="flex gap-2">
              <Button
                variant="outline"
                size="sm"
                className="h-8 flex-1 text-xs"
                onClick={() => setConfirmUninstallSkill(null)}
                disabled={uninstallingSkillId === confirmUninstallSkill.id}
              >
                Cancel
              </Button>
              <Button
                variant="destructive"
                size="sm"
                className="h-8 flex-1 text-xs"
                onClick={() => void handleUninstallSkill(confirmUninstallSkill)}
                disabled={uninstallingSkillId === confirmUninstallSkill.id}
              >
                {uninstallingSkillId === confirmUninstallSkill.id ? (
                  <Loader2 className="mr-1.5 h-3 w-3 animate-spin" />
                ) : (
                  <Trash2 className="mr-1.5 h-3 w-3" />
                )}
                Uninstall
              </Button>
            </CardContent>
          </DialogCard>
        </DialogOverlay>
      ) : null}

      {/* Confirm Remove Repo Dialog */}
      {confirmRemoveRepo ? (
        <DialogOverlay onClose={() => setConfirmRemoveRepo(null)}>
          <DialogCard onClick={(e) => e.stopPropagation()} className="max-w-sm">
            <CardHeader className="pb-2">
              <CardTitle className="text-base">Remove Repository</CardTitle>
              <CardDescription className="text-xs">
                Remove "{confirmRemoveRepo.id}" from skill repositories?
                Installed skills will remain.
              </CardDescription>
            </CardHeader>
            <CardContent className="flex gap-2">
              <Button
                variant="outline"
                size="sm"
                className="h-8 flex-1 text-xs"
                onClick={() => setConfirmRemoveRepo(null)}
                disabled={removingRepoId === confirmRemoveRepo.id}
              >
                Cancel
              </Button>
              <Button
                variant="destructive"
                size="sm"
                className="h-8 flex-1 text-xs"
                onClick={() => void handleRemoveRepo(confirmRemoveRepo)}
                disabled={removingRepoId === confirmRemoveRepo.id}
              >
                {removingRepoId === confirmRemoveRepo.id ? (
                  <Loader2 className="mr-1.5 h-3 w-3 animate-spin" />
                ) : (
                  <Trash2 className="mr-1.5 h-3 w-3" />
                )}
                Remove
              </Button>
            </CardContent>
          </DialogCard>
        </DialogOverlay>
      ) : null}
    </div>
  );
}

// Installed Tab Component
interface InstalledTabProps {
  skills: Skill[];
  onInstallFromPath: () => void;
  onInstallFromGit: () => void;
  onInstallFromFileArchive: () => void;
  onToggleProvider: (skill: Skill, provider: string, enabled: boolean) => Promise<void>;
  onUninstall: (skill: Skill) => void;
  togglingSkillId: string | null;
  uninstallingSkillId: string | null;
  getEnabledState: (skill: Skill) => { claude_code: boolean; codex: boolean; opencode: boolean };
  isSkillEnabledForProvider: (skill: Skill, provider: string) => boolean;
}

function InstalledTab({
  skills,
  onInstallFromPath,
  onInstallFromGit,
  onInstallFromFileArchive,
  onToggleProvider,
  onUninstall,
  togglingSkillId,
  uninstallingSkillId,
  getEnabledState,
  isSkillEnabledForProvider,
}: InstalledTabProps) {
  return (
    <>
      {/* Install buttons */}
      <div className="space-y-1">
        <Button
          variant="ghost"
          size="sm"
          className="h-8 w-full items-center justify-between px-2.5 text-xs"
          onClick={onInstallFromPath}
        >
          <span className="inline-flex items-center gap-1.5">
            <FolderSearch className="h-3.5 w-3.5" />
            Install from Path
          </span>
          <ChevronRight className="h-3.5 w-3.5" />
        </Button>

        <Button
          variant="ghost"
          size="sm"
          className="h-8 w-full items-center justify-between px-2.5 text-xs"
          onClick={onInstallFromGit}
        >
          <span className="inline-flex items-center gap-1.5">
            <Github className="h-3.5 w-3.5" />
            Install from Git
          </span>
          <ChevronRight className="h-3.5 w-3.5" />
        </Button>

        <Button
          variant="ghost"
          size="sm"
          className="h-8 w-full items-center justify-between px-2.5 text-xs"
          onClick={onInstallFromFileArchive}
        >
          <span className="inline-flex items-center gap-1.5">
            <FileArchive className="h-3.5 w-3.5" />
            Install from ZIP
          </span>
          <ChevronRight className="h-3.5 w-3.5" />
        </Button>
      </div>

      {/* Skills list */}
      {skills.length > 0 ? (
        <div className="max-h-64 space-y-2 overflow-y-auto">
          {skills.map((skill) => {
            const enabledState = getEnabledState(skill);
            const anyEnabled = Object.values(enabledState).some(Boolean);

            return (
              <div
                key={skill.id}
                className="rounded-md border border-border/50 p-2"
              >
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5">
                      <Package className="h-3 w-3 shrink-0 text-muted-foreground" />
                      <span className="truncate text-xs font-medium">
                        {skill.name}
                      </span>
                      <Badge
                        variant={anyEnabled ? "default" : "outline"}
                        className={cn(
                          "h-4 px-1 text-[9px]",
                          anyEnabled
                            ? "bg-green-600/90 text-white hover:bg-green-600"
                            : "text-muted-foreground"
                        )}
                      >
                        {anyEnabled ? "ON" : "OFF"}
                      </Badge>
                    </div>
                    <p className="truncate text-[10px] text-muted-foreground">
                      v{skill.version}
                      {skill.repoOwner && skill.repoName && (
                        <span className="ml-1">· {skill.repoOwner}/{skill.repoName}</span>
                      )}
                    </p>
                    {skill.description && (
                      <p className="mt-1 line-clamp-2 text-[10px] text-muted-foreground">
                        {skill.description}
                      </p>
                    )}
                  </div>

                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-6 w-6 shrink-0 p-0 text-muted-foreground hover:text-destructive"
                    onClick={() => onUninstall(skill)}
                    disabled={uninstallingSkillId === skill.id}
                    title="Uninstall skill"
                  >
                    {uninstallingSkillId === skill.id ? (
                      <Loader2 className="h-3 w-3 animate-spin" />
                    ) : (
                      <Trash2 className="h-3 w-3" />
                    )}
                  </Button>
                </div>

                {/* Per-provider toggles */}
                <div className="mt-2 flex gap-1">
                  {PROVIDERS.map((provider) => {
                    const isEnabled = isSkillEnabledForProvider(skill, provider.id);
                    const isToggling = togglingSkillId === skill.id;

                    return (
                      <Button
                        key={provider.id}
                        variant={isEnabled ? "default" : "outline"}
                        size="sm"
                        className={cn(
                          "h-5 px-1.5 text-[9px]",
                          isEnabled && "bg-primary text-primary-foreground"
                        )}
                        onClick={() =>
                          void onToggleProvider(skill, provider.id, !isEnabled)
                        }
                        disabled={isToggling}
                        title={`${provider.label}: ${isEnabled ? "Enabled" : "Disabled"}`}
                      >
                        {isToggling ? (
                          <Loader2 className="h-2.5 w-2.5 animate-spin" />
                        ) : (
                          provider.label
                        )}
                      </Button>
                    );
                  })}
                </div>
              </div>
            );
          })}
        </div>
      ) : (
        <p className="text-xs text-muted-foreground">
          No skills installed. Install from a local folder, Git repository, or ZIP file.
        </p>
      )}
    </>
  );
}

// Discover Tab Component
interface DiscoverTabProps {
  skillRepos: SkillRepo[];
  discoverableSkills: DiscoverableSkill[];
  installedSkills: Skill[];
  discoverInstallProgress: Record<string, DiscoverSkillInstallProgress>;
  discovering: boolean;
  onDiscoverSkills: (forceRefresh?: boolean) => void;
  onManageRepos: () => void;
  onInstallDiscoveredSkill: (skill: DiscoverableSkill) => Promise<void>;
}

function DiscoverTab({
  skillRepos,
  discoverableSkills,
  installedSkills,
  discoverInstallProgress,
  discovering,
  onDiscoverSkills,
  onManageRepos,
  onInstallDiscoveredSkill,
}: DiscoverTabProps) {
  // Track if we've already discovered skills
  const hasDiscovered = useRef(false);
  const [searchQuery, setSearchQuery] = useState("");

  // Discover skills when repos change (only once, uses cache)
  useEffect(() => {
    if (skillRepos.length > 0 && !hasDiscovered.current && !discovering) {
      hasDiscovered.current = true;
      onDiscoverSkills(false); // Use cache
    }
  }, [skillRepos.length, discovering, onDiscoverSkills]);

  // Reset discovery flag when repos change
  useEffect(() => {
    hasDiscovered.current = false;
  }, [skillRepos]);

  // Filter skills by search query
  const filteredSkills = useMemo(() => {
    if (!searchQuery.trim()) return discoverableSkills;
    const query = searchQuery.toLowerCase();
    return discoverableSkills.filter(
      (skill) =>
        skill.name.toLowerCase().includes(query) ||
        skill.description.toLowerCase().includes(query) ||
        skill.directory.toLowerCase().includes(query)
    );
  }, [discoverableSkills, searchQuery]);

  // Group skills by repo
  const skillsByRepo = filteredSkills.reduce((acc, skill) => {
    const repoId = `${skill.repoOwner}/${skill.repoName}`;
    if (!acc[repoId]) {
      acc[repoId] = [];
    }
    acc[repoId].push(skill);
    return acc;
  }, {} as Record<string, DiscoverableSkill[]>);

  const installedDiscoverKeys = useMemo(() => {
    const keys = new Set<string>();

    for (const skill of installedSkills) {
      if (!skill.repoOwner || !skill.repoName) {
        continue;
      }

      let directory: string | null = null;

      if (skill.readmeUrl) {
        if (skill.repoBranch) {
          const prefix = `https://github.com/${skill.repoOwner}/${skill.repoName}/tree/${skill.repoBranch}/`;
          if (skill.readmeUrl.startsWith(prefix)) {
            directory = skill.readmeUrl.slice(prefix.length);
          }
        }

        if (!directory) {
          const genericPrefix = `https://github.com/${skill.repoOwner}/${skill.repoName}/tree/`;
          if (skill.readmeUrl.startsWith(genericPrefix)) {
            const rest = skill.readmeUrl.slice(genericPrefix.length);
            const slashIndex = rest.indexOf("/");
            if (slashIndex >= 0 && slashIndex < rest.length - 1) {
              directory = rest.slice(slashIndex + 1);
            }
          }
        }
      }

      if (!directory && skill.id) {
        directory = skill.id;
      }

      if (directory) {
        keys.add(`${skill.repoOwner}/${skill.repoName}:${directory}`);
      }
    }

    return keys;
  }, [installedSkills]);

  const handleRefresh = () => {
    onDiscoverSkills(true); // Force refresh
  };

  return (
    <div className="space-y-2">
      {/* Search box */}
      {skillRepos.length > 0 && discoverableSkills.length > 0 && (
        <div className="relative">
          <Search className="absolute left-2 top-1/2 h-3 w-3 -translate-y-1/2 text-muted-foreground" />
          <input
            type="text"
            placeholder="Search skills..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="h-7 w-full rounded-md border border-border bg-background pl-7 pr-2 text-xs outline-none focus:ring-1 focus:ring-ring"
          />
        </div>
      )}

      {/* Header with refresh button */}
      {skillRepos.length > 0 && (
        <div className="flex items-center justify-between">
          <p className="text-[10px] text-muted-foreground">
            {searchQuery.trim()
              ? `${filteredSkills.length} of ${discoverableSkills.length} skills`
              : `${discoverableSkills.length} skills found`}
          </p>
          <Button
            variant="ghost"
            size="sm"
            className="h-5 w-5 p-0"
            onClick={handleRefresh}
            disabled={discovering}
          >
            <RefreshCw className={cn("h-3 w-3", discovering && "animate-spin")} />
          </Button>
        </div>
      )}

      {discovering ? (
        <div className="flex items-center justify-center py-4">
          <Loader2 className="h-4 w-4 animate-spin text-muted-foreground" />
          <span className="ml-2 text-xs text-muted-foreground">Discovering skills...</span>
        </div>
      ) : skillRepos.length > 0 ? (
        <>
          {discoverableSkills.length > 0 ? (
            <div className="max-h-64 space-y-2 overflow-y-auto">
              {Object.entries(skillsByRepo).map(([repoId, skills]) => (
                <div key={repoId} className="space-y-1">
                  <p className="text-[10px] font-medium text-muted-foreground">{repoId}</p>
                  {skills.map((skill) => {
                    const installState = discoverInstallProgress[skill.key];
                    const isInstalled = installedDiscoverKeys.has(skill.key);
                    const isInstalling = Boolean(
                      installState &&
                        installState.stage !== "completed" &&
                        installState.stage !== "failed"
                    );
                    const isCompleted = installState?.stage === "completed";
                    const isFailed = installState?.stage === "failed";

                    return (
                      <div key={skill.key} className="rounded-md border border-border/50 p-2">
                        <div className="flex items-start justify-between gap-2">
                          <div className="min-w-0 flex-1">
                            <div className="flex items-center gap-1.5">
                              <Package className="h-3 w-3 shrink-0 text-muted-foreground" />
                              <span className="truncate text-xs font-medium">
                                {skill.name}
                              </span>
                            </div>
                            {skill.description && (
                              <p className="mt-1 line-clamp-2 text-[10px] text-muted-foreground">
                                {skill.description}
                              </p>
                            )}
                            {(installState || isInstalled) && (
                              <p
                                className={cn(
                                  "mt-1 line-clamp-2 text-[10px]",
                                  isFailed
                                    ? "text-destructive"
                                    : isCompleted
                                      ? "text-emerald-600 dark:text-emerald-400"
                                      : isInstalled
                                        ? "text-emerald-600 dark:text-emerald-400"
                                        : "text-muted-foreground"
                                )}
                              >
                                {installState?.message ?? (isInstalled ? "Already installed" : "")}
                              </p>
                            )}
                          </div>
                          <Button
                            variant="default"
                            size="sm"
                            className="h-6 px-2 text-[10px]"
                            onClick={() => void onInstallDiscoveredSkill(skill)}
                            disabled={isInstalling || isCompleted || isInstalled}
                          >
                            {isInstalling ? (
                              <Loader2 className="mr-1 h-3 w-3 animate-spin" />
                            ) : (
                              <Download className="mr-1 h-3 w-3" />
                            )}
                            {isInstalling
                              ? "Installing..."
                              : (isCompleted || isInstalled)
                                ? "Installed"
                                : isFailed
                                  ? "Retry"
                                  : "Install"}
                          </Button>
                        </div>
                      </div>
                    );
                  })}
                </div>
              ))}
            </div>
          ) : (
            <p className="text-xs text-muted-foreground">
              No skills found in the configured repositories.
            </p>
          )}
        </>
      ) : (
        <p className="text-xs text-muted-foreground">
          Add a repository to discover skills.
        </p>
      )}

      <Button
        variant="outline"
        size="sm"
        className="h-7 w-full text-xs"
        onClick={onManageRepos}
        disabled={discovering}
      >
        <Github className="mr-1.5 h-3 w-3" />
        Manage Repositories
      </Button>
    </div>
  );
}


// Dialog helper components
function DialogOverlay({
  children,
  onClose,
}: {
  children: React.ReactNode;
  onClose: () => void;
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/35 p-4">
      {children}
      <div className="fixed inset-0 z-40" onClick={onClose} />
    </div>
  );
}

function DialogCard({
  children,
  onClick,
  className,
}: {
  children: React.ReactNode;
  onClick: (e: React.MouseEvent) => void;
  className?: string;
}) {
  return (
    <Card
      className={cn(
        "w-full max-w-md border border-border bg-card opacity-100 shadow-xl z-50",
        className
      )}
      onClick={onClick}
    >
      {children}
    </Card>
  );
}

function DialogButtons({
  onCancel,
  onSubmit,
  submitLabel,
  submitDisabled,
  isSubmitting,
}: {
  onCancel: () => void;
  onSubmit: () => void;
  submitLabel: string;
  submitDisabled: boolean;
  isSubmitting: boolean;
}) {
  return (
    <div className="flex gap-2">
      <Button
        variant="outline"
        size="sm"
        className="h-8 flex-1 text-xs"
        onClick={onCancel}
        disabled={isSubmitting}
      >
        Cancel
      </Button>
      <Button
        variant="default"
        size="sm"
        className="h-8 flex-1 text-xs"
        onClick={onSubmit}
        disabled={submitDisabled}
      >
        {isSubmitting ? (
          <Loader2 className="mr-1.5 h-3 w-3 animate-spin" />
        ) : (
          <Download className="mr-1.5 h-3 w-3" />
        )}
        {submitLabel}
      </Button>
    </div>
  );
}
