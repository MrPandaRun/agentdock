import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";

import type {
  DiscoverableSkill,
  DiscoverSkillInstallProgress,
  ProviderSkill,
  Skill,
  SkillEnabledState,
  SkillRepo,
} from "@/types";

export interface UseSkillsResult {
  skills: Skill[];
  skillRepos: SkillRepo[];
  discoverableSkills: DiscoverableSkill[];
  discoverInstallProgress: Record<string, DiscoverSkillInstallProgress>;
  providerSkills: ProviderSkill[];
  loading: boolean;
  discovering: boolean;
  scanning: boolean;
  error: string | null;
  loadSkills: () => Promise<void>;
  loadSkillRepos: () => Promise<void>;
  discoverSkills: (forceRefresh?: boolean) => Promise<void>;
  scanProviderSkills: () => Promise<void>;
  importProviderSkills: (skillKeys: string[]) => Promise<Skill[]>;
  installSkillFromPath: (path: string) => Promise<Skill>;
  installSkillFromGit: (url: string) => Promise<Skill>;
  installDiscoveredSkill: (skill: DiscoverableSkill) => Promise<Skill>;
  toggleSkill: (id: string, enabled: boolean) => Promise<void>;
  toggleSkillForProvider: (
    id: string,
    provider: string,
    enabled: boolean
  ) => Promise<void>;
  uninstallSkill: (id: string) => Promise<void>;
  addSkillRepo: (owner: string, name: string, branch?: string) => Promise<SkillRepo>;
  removeSkillRepo: (id: string) => Promise<void>;
  getEnabledState: (skill: Skill) => SkillEnabledState;
  isSkillEnabledForProvider: (skill: Skill, provider: string) => boolean;
}

export function useSkills(): UseSkillsResult {
  const [skills, setSkills] = useState<Skill[]>([]);
  const [skillRepos, setSkillRepos] = useState<SkillRepo[]>([]);
  const [discoverableSkills, setDiscoverableSkills] = useState<DiscoverableSkill[]>([]);
  const [discoverInstallProgress, setDiscoverInstallProgress] = useState<
    Record<string, DiscoverSkillInstallProgress>
  >({});
  const [providerSkills, setProviderSkills] = useState<ProviderSkill[]>([]);
  const [loading, setLoading] = useState(true);
  const [discovering, setDiscovering] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const discoverInstallTimeoutsRef = useRef<Record<string, number>>({});

  const scheduleClearDiscoverInstallProgress = useCallback(
    (skillKey: string, delayMs: number): void => {
      const existing = discoverInstallTimeoutsRef.current[skillKey];
      if (existing) {
        window.clearTimeout(existing);
      }

      discoverInstallTimeoutsRef.current[skillKey] = window.setTimeout(() => {
        setDiscoverInstallProgress((prev) => {
          if (!(skillKey in prev)) {
            return prev;
          }
          const next = { ...prev };
          delete next[skillKey];
          return next;
        });
        delete discoverInstallTimeoutsRef.current[skillKey];
      }, delayMs);
    },
    []
  );

  const getEnabledState = useCallback((skill: Skill): SkillEnabledState => {
    if (!skill.enabledJson || skill.enabledJson === "{}") {
      return { claude_code: true, codex: true, opencode: true };
    }
    try {
      return JSON.parse(skill.enabledJson) as SkillEnabledState;
    } catch {
      return { claude_code: true, codex: true, opencode: true };
    }
  }, []);

  const isSkillEnabledForProvider = useCallback(
    (skill: Skill, provider: string): boolean => {
      const state = getEnabledState(skill);
      return state[provider as keyof SkillEnabledState] ?? false;
    },
    [getEnabledState]
  );

  const loadSkills = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await invoke<Skill[]>("list_skills");
      setSkills(data);
    } catch (loadError) {
      const message =
        loadError instanceof Error ? loadError.message : String(loadError);
      setError(message);
    } finally {
      setLoading(false);
    }
  }, []);

  const loadSkillRepos = useCallback(async () => {
    try {
      const data = await invoke<SkillRepo[]>("list_skill_repos");
      setSkillRepos(data);
    } catch (loadError) {
      const message =
        loadError instanceof Error ? loadError.message : String(loadError);
      setError(message);
    }
  }, []);

  const installSkillFromPath = useCallback(
    async (path: string): Promise<Skill> => {
      setError(null);
      try {
        const skill = await invoke<Skill>("install_skill_from_path", {
          request: { path },
        });
        setSkills((prev) => {
          const filtered = prev.filter((s) => s.id !== skill.id);
          return [...filtered, skill];
        });
        return skill;
      } catch (installError) {
        const message =
          installError instanceof Error
            ? installError.message
            : String(installError);
        setError(message);
        throw new Error(message);
      }
    },
    []
  );

  const installSkillFromGit = useCallback(
    async (url: string): Promise<Skill> => {
      setError(null);
      try {
        const skill = await invoke<Skill>("install_skill_from_git", {
          request: { url },
        });
        setSkills((prev) => {
          const filtered = prev.filter((s) => s.id !== skill.id);
          return [...filtered, skill];
        });
        return skill;
      } catch (installError) {
        const message =
          installError instanceof Error
            ? installError.message
            : String(installError);
        setError(message);
        throw new Error(message);
      }
    },
    []
  );

  const installDiscoveredSkill = useCallback(
    async (skill: DiscoverableSkill): Promise<Skill> => {
      setError(null);
      setDiscoverInstallProgress((prev) => ({
        ...prev,
        [skill.key]: {
          key: skill.key,
          stage: "queued",
          message: "Queued for installation...",
        },
      }));
      try {
        const result = await invoke<Skill>("install_discovered_skill", {
          request: { skill },
        });
        setSkills((prev) => {
          const filtered = prev.filter((s) => s.id !== result.id);
          return [...filtered, result];
        });
        setDiscoverInstallProgress((prev) => ({
          ...prev,
          [skill.key]: {
            key: skill.key,
            stage: "completed",
            message: "Installed successfully",
          },
        }));
        scheduleClearDiscoverInstallProgress(skill.key, 1500);
        return result;
      } catch (installError) {
        const message =
          installError instanceof Error
            ? installError.message
            : String(installError);
        setDiscoverInstallProgress((prev) => ({
          ...prev,
          [skill.key]: {
            key: skill.key,
            stage: "failed",
            message,
          },
        }));
        scheduleClearDiscoverInstallProgress(skill.key, 4500);
        throw new Error(message);
      }
    },
    [scheduleClearDiscoverInstallProgress]
  );

  const toggleSkill = useCallback(
    async (id: string, enabled: boolean): Promise<void> => {
      setError(null);
      try {
        await invoke("toggle_skill_enabled", {
          request: { id, enabled },
        });
        setSkills((prev) => {
          const newState = enabled
            ? { claude_code: true, codex: true, opencode: true }
            : { claude_code: false, codex: false, opencode: false };
          return prev.map((s) =>
            s.id === id
              ? { ...s, enabledJson: JSON.stringify(newState) }
              : s
          );
        });
      } catch (toggleError) {
        const message =
          toggleError instanceof Error
            ? toggleError.message
            : String(toggleError);
        setError(message);
        throw new Error(message);
      }
    },
    []
  );

  const toggleSkillForProvider = useCallback(
    async (id: string, provider: string, enabled: boolean): Promise<void> => {
      setError(null);
      try {
        await invoke("toggle_skill_enabled_for_provider", {
          request: { id, provider, enabled },
        });
        setSkills((prev) =>
          prev.map((s) => {
            if (s.id !== id) return s;
            const currentState = getEnabledState(s);
            const newState = { ...currentState, [provider]: enabled };
            return { ...s, enabledJson: JSON.stringify(newState) };
          })
        );
      } catch (toggleError) {
        const message =
          toggleError instanceof Error
            ? toggleError.message
            : String(toggleError);
        setError(message);
        throw new Error(message);
      }
    },
    [getEnabledState]
  );

  const uninstallSkill = useCallback(async (id: string): Promise<void> => {
    setError(null);
    try {
      await invoke("uninstall_skill", {
        request: { id },
      });
      setSkills((prev) => prev.filter((s) => s.id !== id));
    } catch (uninstallError) {
      const message =
        uninstallError instanceof Error
          ? uninstallError.message
          : String(uninstallError);
      setError(message);
      throw new Error(message);
    }
  }, []);

  const addSkillRepo = useCallback(
    async (owner: string, name: string, branch?: string): Promise<SkillRepo> => {
      setError(null);
      try {
        const repo = await invoke<SkillRepo>("add_skill_repo", {
          request: { owner, name, branch },
        });
        setSkillRepos((prev) => {
          const filtered = prev.filter((r) => r.id !== repo.id);
          return [...filtered, repo];
        });
        return repo;
      } catch (addError) {
        const message =
          addError instanceof Error ? addError.message : String(addError);
        setError(message);
        throw new Error(message);
      }
    },
    []
  );

  const removeSkillRepo = useCallback(async (id: string): Promise<void> => {
    setError(null);
    try {
      await invoke("remove_skill_repo", {
        request: { id },
      });
      setSkillRepos((prev) => prev.filter((r) => r.id !== id));
    } catch (removeError) {
      const message =
        removeError instanceof Error
          ? removeError.message
          : String(removeError);
      setError(message);
      throw new Error(message);
    }
  }, []);

  const discoverSkills = useCallback(async (forceRefresh = false): Promise<void> => {
    setDiscovering(true);
    setError(null);
    try {
      const data = await invoke<DiscoverableSkill[]>("discover_skills", {
        forceRefresh,
      });
      setDiscoverableSkills(data);
    } catch (discoverError) {
      const message =
        discoverError instanceof Error
          ? discoverError.message
          : String(discoverError);
      setError(message);
    } finally {
      setDiscovering(false);
    }
  }, []);

  const scanProviderSkills = useCallback(async (): Promise<void> => {
    setScanning(true);
    setError(null);
    try {
      const data = await invoke<ProviderSkill[]>("scan_provider_skills");
      setProviderSkills(data);
    } catch (scanError) {
      const message =
        scanError instanceof Error ? scanError.message : String(scanError);
      setError(message);
    } finally {
      setScanning(false);
    }
  }, []);

  const importProviderSkills = useCallback(
    async (skillKeys: string[]): Promise<Skill[]> => {
    setError(null);
    try {
      const imported = await invoke<Skill[]>("import_provider_skills", {
        request: { skill_keys: skillKeys },
      });
      // Reload skills to get updated list
      await loadSkills();
      // Remove imported skills from provider skills list
      setProviderSkills((prev) =>
        prev.filter((s) => !skillKeys.includes(s.key))
      );
      return imported;
    } catch (importError) {
      const message =
        importError instanceof Error
          ? importError.message
          : String(importError);
      setError(message);
      throw new Error(message);
    }
  },
    [loadSkills]
  );

  useEffect(() => {
    void loadSkills();
    void loadSkillRepos();
  }, [loadSkills, loadSkillRepos]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let active = true;

    void (async () => {
      unlisten = await listen<DiscoverSkillInstallProgress>(
        "discover-skill-install-progress",
        (event) => {
          const progress = event.payload;
          setDiscoverInstallProgress((prev) => ({
            ...prev,
            [progress.key]: progress,
          }));

          if (progress.stage === "completed") {
            scheduleClearDiscoverInstallProgress(progress.key, 1500);
          } else if (progress.stage === "failed") {
            scheduleClearDiscoverInstallProgress(progress.key, 4500);
          }
        }
      );

      if (!active && unlisten) {
        unlisten();
        unlisten = null;
      }
    })();

    return () => {
      active = false;
      if (unlisten) {
        unlisten();
      }
    };
  }, [scheduleClearDiscoverInstallProgress]);

  useEffect(() => {
    return () => {
      Object.values(discoverInstallTimeoutsRef.current).forEach((timeoutId) => {
        window.clearTimeout(timeoutId);
      });
      discoverInstallTimeoutsRef.current = {};
    };
  }, []);

  return {
    skills,
    skillRepos,
    discoverableSkills,
    discoverInstallProgress,
    providerSkills,
    loading,
    discovering,
    scanning,
    error,
    loadSkills,
    loadSkillRepos,
    discoverSkills,
    scanProviderSkills,
    importProviderSkills,
    installSkillFromPath,
    installSkillFromGit,
    installDiscoveredSkill,
    toggleSkill,
    toggleSkillForProvider,
    uninstallSkill,
    addSkillRepo,
    removeSkillRepo,
    getEnabledState,
    isSkillEnabledForProvider,
  };
}
