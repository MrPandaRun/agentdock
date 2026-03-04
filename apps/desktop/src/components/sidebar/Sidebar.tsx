import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  AlertTriangle,
  Check,
  ChevronRight,
  ChevronUp,
  ExternalLink,
  Folder,
  FolderSearch,
  Loader2,
  Monitor,
  Moon,
  Package,
  Plus,
  RefreshCw,
  Settings2,
  Sun,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { ProviderIcon } from "@/components/provider/ProviderIcon";
import { SkillsPanel } from "@/components/skills/SkillsPanel";
import {
  ThreadFolderGroup,
  type ThreadFolderGroupItem,
} from "@/components/threads/ThreadFolderGroup";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { JsonCodeEditor } from "@/components/ui/json-code-editor";
import {
  isSupportedProvider,
  providerDisplayName,
  providerInstallGuideUrl,
} from "@/lib/provider";
import { normalizeProjectPath, formatLastActive, threadPreview } from "@/lib/thread";
import { cn } from "@/lib/utils";
import type {
  AgentRuntimeSettings,
  AgentSupplier,
  AppTheme,
  ProviderInstallStatus,
  ProviderProfileMap,
  ThreadProviderId,
} from "@/types";

interface AppThemeOption {
  value: AppTheme;
  label: string;
  Icon: typeof Sun;
}

interface ProviderOption {
  value: ThreadProviderId;
  label: string;
  accentClass: string;
}

interface ProviderInstallStatusPayload {
  providerId: string;
  installed: boolean;
  healthStatus: string;
  message?: string | null;
}

interface CcSwitchImportedSupplierPayload {
  providerId: string;
  sourceId: string;
  name: string;
  note?: string | null;
  profileName: string;
  baseUrl?: string | null;
  apiKey?: string | null;
  configJson?: string | null;
  isCurrent: boolean;
}

interface CcSwitchImportPayload {
  dbPath: string;
  suppliers: CcSwitchImportedSupplierPayload[];
}

type SupplierConfigProtocol =
  | "claude_env"
  | "codex_auth_config"
  | "opencode_settings";

interface SupplierPreset {
  id: string;
  providerId: ThreadProviderId;
  label: string;
  description: string;
  name: string;
  note?: string;
  profileName: string;
  baseUrl?: string;
  configJson?: string;
  configProtocol?: SupplierConfigProtocol;
  opencodeNpm?: string;
  docsUrl?: string;
}

interface JsonValidationResult {
  isValid: boolean;
  message: string;
}

interface ConfigEditorNotice {
  tone: "success" | "error";
  message: string;
}

const APP_THEME_OPTIONS: AppThemeOption[] = [
  { value: "light", label: "Light", Icon: Sun },
  { value: "dark", label: "Dark", Icon: Moon },
  { value: "system", label: "System", Icon: Monitor },
];

const THREAD_PROVIDER_OPTIONS: ProviderOption[] = [
  {
    value: "claude_code",
    label: "Claude Code",
    accentClass: "text-[#FF7043] dark:text-[#FF8A65]",
  },
  {
    value: "codex",
    label: "Codex",
    accentClass: "text-[#111111] dark:text-[#F2F2F2]",
  },
  {
    value: "opencode",
    label: "OpenCode",
    accentClass: "text-[#211E1E] dark:text-[#F1ECEC]",
  },
];
const OFFICIAL_SUPPLIER_ID = "official-default";
const SUPPLIER_PRESETS: Record<ThreadProviderId, SupplierPreset[]> = {
  claude_code: [
    {
      id: "manual-custom-empty",
      providerId: "claude_code",
      label: "Custom (Empty)",
      description: "Empty template. Fill all fields manually.",
      name: "Custom",
      note: "Manual configuration template",
      profileName: "default",
    },
    {
      id: "zhipu-glm-anthropic",
      providerId: "claude_code",
      label: "Zhipu GLM",
      description: "Anthropic-compatible endpoint for Claude Code.",
      name: "Zhipu GLM",
      profileName: "zhipu-glm",
      baseUrl: "https://open.bigmodel.cn/api/anthropic",
      configJson: JSON.stringify(
        {
          env: {
            ANTHROPIC_DEFAULT_HAIKU_MODEL: "glm-4.5-air",
            ANTHROPIC_DEFAULT_SONNET_MODEL: "glm-4.7",
            ANTHROPIC_DEFAULT_OPUS_MODEL: "glm-5",
            API_TIMEOUT_MS: "300000",
          },
        },
        null,
        2,
      ),
      docsUrl: "https://open.bigmodel.cn/dev/api#anthropic-messages",
    },
    {
      id: "claude-deepseek",
      providerId: "claude_code",
      label: "DeepSeek",
      description: "DeepSeek Anthropic-compatible endpoint.",
      name: "DeepSeek",
      profileName: "deepseek",
      baseUrl: "https://api.deepseek.com/anthropic",
      docsUrl: "https://platform.deepseek.com",
    },
    {
      id: "claude-zai-glm",
      providerId: "claude_code",
      label: "Z.ai GLM",
      description: "Z.ai GLM Anthropic-compatible endpoint.",
      name: "Z.ai GLM",
      profileName: "zai-glm",
      baseUrl: "https://api.z.ai/api/anthropic",
      docsUrl: "https://z.ai",
    },
    {
      id: "claude-qwen-coder",
      providerId: "claude_code",
      label: "Qwen Coder",
      description: "DashScope Anthropic-compatible endpoint.",
      name: "Qwen Coder",
      profileName: "qwen-coder",
      baseUrl: "https://dashscope.aliyuncs.com/apps/anthropic",
      docsUrl: "https://bailian.console.aliyun.com",
    },
    {
      id: "claude-kimi-k2",
      providerId: "claude_code",
      label: "Kimi k2",
      description: "Moonshot Anthropic-compatible endpoint.",
      name: "Kimi k2",
      profileName: "kimi-k2",
      baseUrl: "https://api.moonshot.cn/anthropic",
      docsUrl: "https://platform.moonshot.cn/console",
    },
    {
      id: "claude-kimi-coding",
      providerId: "claude_code",
      label: "Kimi For Coding",
      description: "Kimi coding endpoint.",
      name: "Kimi For Coding",
      profileName: "kimi-coding",
      baseUrl: "https://api.kimi.com/coding/",
      docsUrl: "https://www.kimi.com/coding/docs/",
    },
    {
      id: "claude-modelscope",
      providerId: "claude_code",
      label: "ModelScope",
      description: "ModelScope Anthropic-compatible endpoint.",
      name: "ModelScope",
      profileName: "modelscope",
      baseUrl: "https://api-inference.modelscope.cn",
      docsUrl: "https://modelscope.cn",
    },
    {
      id: "claude-kat-coder",
      providerId: "claude_code",
      label: "KAT-Coder",
      description: "KAT-Coder gateway template; replace ENDPOINT_ID.",
      name: "KAT-Coder",
      profileName: "kat-coder",
      baseUrl: "https://vanchin.streamlake.ai/api/gateway/v1/endpoints/${ENDPOINT_ID}/claude-code-proxy",
      docsUrl: "https://console.streamlake.ai",
    },
    {
      id: "claude-longcat",
      providerId: "claude_code",
      label: "Longcat",
      description: "Longcat Anthropic-compatible endpoint.",
      name: "Longcat",
      profileName: "longcat",
      baseUrl: "https://api.longcat.chat/anthropic",
      docsUrl: "https://longcat.chat/platform",
    },
    {
      id: "claude-minimax-cn",
      providerId: "claude_code",
      label: "MiniMax",
      description: "MiniMax CN Anthropic-compatible endpoint.",
      name: "MiniMax",
      profileName: "minimax-cn",
      baseUrl: "https://api.minimaxi.com/anthropic",
      docsUrl: "https://platform.minimaxi.com",
    },
    {
      id: "claude-minimax-en",
      providerId: "claude_code",
      label: "MiniMax en",
      description: "MiniMax EN Anthropic-compatible endpoint.",
      name: "MiniMax en",
      profileName: "minimax-en",
      baseUrl: "https://api.minimax.io/anthropic",
      docsUrl: "https://platform.minimax.io",
    },
    {
      id: "claude-doubao-seed",
      providerId: "claude_code",
      label: "DouBaoSeed",
      description: "DouBao coding endpoint.",
      name: "DouBaoSeed",
      profileName: "doubao-seed",
      baseUrl: "https://ark.cn-beijing.volces.com/api/coding",
      docsUrl: "https://www.volcengine.com/product/doubao",
    },
    {
      id: "claude-bailing",
      providerId: "claude_code",
      label: "BaiLing",
      description: "BaiLing Anthropic-compatible endpoint.",
      name: "BaiLing",
      profileName: "bailing",
      baseUrl: "https://api.tbox.cn/api/anthropic",
      docsUrl: "https://alipaytbox.yuque.com/sxs0ba/ling/get_started",
    },
    {
      id: "claude-aihubmix",
      providerId: "claude_code",
      label: "AiHubMix",
      description: "AiHubMix endpoint.",
      name: "AiHubMix",
      profileName: "aihubmix",
      baseUrl: "https://aihubmix.com",
      docsUrl: "https://aihubmix.com",
    },
    {
      id: "claude-dmxapi",
      providerId: "claude_code",
      label: "DMXAPI",
      description: "DMXAPI endpoint.",
      name: "DMXAPI",
      profileName: "dmxapi",
      baseUrl: "https://www.dmxapi.cn",
      docsUrl: "https://www.dmxapi.cn",
    },
    {
      id: "claude-packycode",
      providerId: "claude_code",
      label: "PackyCode",
      description: "PackyCode endpoint.",
      name: "PackyCode",
      profileName: "packycode",
      baseUrl: "https://www.packyapi.com",
      docsUrl: "https://www.packyapi.com",
    },
    {
      id: "claude-cubence",
      providerId: "claude_code",
      label: "Cubence",
      description: "Cubence endpoint.",
      name: "Cubence",
      profileName: "cubence",
      baseUrl: "https://api.cubence.com",
      docsUrl: "https://cubence.com",
    },
    {
      id: "claude-aigocode",
      providerId: "claude_code",
      label: "AIGoCode",
      description: "AIGoCode endpoint.",
      name: "AIGoCode",
      profileName: "aigocode",
      baseUrl: "https://api.aigocode.com/api",
      docsUrl: "https://aigocode.com",
    },
    {
      id: "claude-openrouter",
      providerId: "claude_code",
      label: "OpenRouter",
      description: "OpenRouter Anthropic-compatible endpoint.",
      name: "OpenRouter",
      profileName: "openrouter",
      baseUrl: "https://openrouter.ai/api",
      docsUrl: "https://openrouter.ai",
    },
    {
      id: "claude-xiaomi-mimo",
      providerId: "claude_code",
      label: "Xiaomi MiMo",
      description: "Xiaomi MiMo Anthropic-compatible endpoint.",
      name: "Xiaomi MiMo",
      profileName: "xiaomi-mimo",
      baseUrl: "https://api.xiaomimimo.com/anthropic",
      docsUrl: "https://platform.xiaomimimo.com",
    },
  ],
  codex: [
    {
      id: "manual-custom-empty",
      providerId: "codex",
      label: "Custom (Empty)",
      description: "Empty template. Fill all fields manually.",
      name: "Custom",
      note: "Manual configuration template",
      profileName: "default",
    },
    {
      id: "codex-openai-official",
      providerId: "codex",
      label: "OpenAI Official",
      description: "Official OpenAI Codex endpoint.",
      name: "OpenAI Official",
      profileName: "openai-official",
      docsUrl: "https://chatgpt.com/codex",
    },
    {
      id: "codex-azure-openai",
      providerId: "codex",
      label: "Azure OpenAI",
      description: "Azure OpenAI Codex endpoint template.",
      name: "Azure OpenAI",
      profileName: "azure-openai",
      baseUrl: "https://YOUR_RESOURCE_NAME.openai.azure.com/openai",
      docsUrl: "https://learn.microsoft.com/en-us/azure/ai-foundry/openai/how-to/codex",
    },
    {
      id: "codex-aihubmix",
      providerId: "codex",
      label: "AiHubMix",
      description: "AiHubMix OpenAI-compatible endpoint.",
      name: "AiHubMix",
      profileName: "aihubmix",
      baseUrl: "https://aihubmix.com/v1",
      docsUrl: "https://aihubmix.com",
    },
    {
      id: "codex-dmxapi",
      providerId: "codex",
      label: "DMXAPI",
      description: "DMXAPI OpenAI-compatible endpoint.",
      name: "DMXAPI",
      profileName: "dmxapi",
      baseUrl: "https://www.dmxapi.cn/v1",
      docsUrl: "https://www.dmxapi.cn",
    },
    {
      id: "codex-packycode",
      providerId: "codex",
      label: "PackyCode",
      description: "PackyCode OpenAI-compatible endpoint.",
      name: "PackyCode",
      profileName: "packycode",
      baseUrl: "https://www.packyapi.com/v1",
      docsUrl: "https://www.packyapi.com",
    },
    {
      id: "codex-cubence",
      providerId: "codex",
      label: "Cubence",
      description: "Cubence OpenAI-compatible endpoint.",
      name: "Cubence",
      profileName: "cubence",
      baseUrl: "https://api.cubence.com/v1",
      docsUrl: "https://cubence.com",
    },
    {
      id: "codex-aigocode",
      providerId: "codex",
      label: "AIGoCode",
      description: "AIGoCode OpenAI-compatible endpoint.",
      name: "AIGoCode",
      profileName: "aigocode",
      baseUrl: "https://api.aigocode.com",
      docsUrl: "https://aigocode.com",
    },
    {
      id: "codex-rightcode",
      providerId: "codex",
      label: "RightCode",
      description: "RightCode Codex-compatible endpoint.",
      name: "RightCode",
      profileName: "rightcode",
      baseUrl: "https://right.codes/codex/v1",
      docsUrl: "https://www.right.codes",
    },
    {
      id: "codex-aicodemirror",
      providerId: "codex",
      label: "AICodeMirror",
      description: "AICodeMirror Codex-compatible endpoint.",
      name: "AICodeMirror",
      profileName: "aicodemirror",
      baseUrl: "https://api.aicodemirror.com/api/codex/backend-api/codex",
      docsUrl: "https://www.aicodemirror.com",
    },
    {
      id: "codex-aicoding",
      providerId: "codex",
      label: "AICoding",
      description: "AICoding Codex-compatible endpoint.",
      name: "AICoding",
      profileName: "aicoding",
      baseUrl: "https://api.aicoding.sh",
      docsUrl: "https://www.aicoding.sh",
    },
    {
      id: "codex-crazyrouter",
      providerId: "codex",
      label: "CrazyRouter",
      description: "CrazyRouter Codex-compatible endpoint.",
      name: "CrazyRouter",
      profileName: "crazyrouter",
      baseUrl: "https://crazyrouter.com/v1",
      docsUrl: "https://www.crazyrouter.com",
    },
    {
      id: "codex-sssaicode",
      providerId: "codex",
      label: "SSSAiCode",
      description: "SSSAiCode Codex-compatible endpoint.",
      name: "SSSAiCode",
      profileName: "sssaicode",
      baseUrl: "https://node-hk.sssaicode.com/api/v1",
      docsUrl: "https://www.sssaicode.com",
    },
    {
      id: "openrouter-openai",
      providerId: "codex",
      label: "OpenRouter",
      description: "OpenAI-compatible gateway with multi-model routing.",
      name: "OpenRouter",
      profileName: "openrouter",
      baseUrl: "https://openrouter.ai/api/v1",
      docsUrl: "https://openrouter.ai",
    },
  ],
  opencode: [
    {
      id: "manual-custom-empty",
      providerId: "opencode",
      label: "Custom (Empty)",
      description: "Empty template. Fill all fields manually.",
      name: "Custom",
      note: "Manual configuration template",
      profileName: "default",
    },
    {
      id: "opencode-deepseek",
      providerId: "opencode",
      label: "DeepSeek",
      description: "DeepSeek OpenCode-compatible endpoint.",
      name: "DeepSeek",
      profileName: "deepseek",
      baseUrl: "https://api.deepseek.com/v1",
      docsUrl: "https://platform.deepseek.com",
    },
    {
      id: "opencode-zhipu-glm",
      providerId: "opencode",
      label: "Zhipu GLM",
      description: "Zhipu GLM OpenCode-compatible endpoint.",
      name: "Zhipu GLM",
      profileName: "zhipu-glm",
      baseUrl: "https://open.bigmodel.cn/api/paas/v4",
      docsUrl: "https://open.bigmodel.cn",
    },
    {
      id: "opencode-zhipu-glm-en",
      providerId: "opencode",
      label: "Zhipu GLM en",
      description: "Zhipu GLM EN OpenCode-compatible endpoint.",
      name: "Zhipu GLM en",
      profileName: "zhipu-glm-en",
      baseUrl: "https://api.z.ai/v1",
      docsUrl: "https://z.ai",
    },
    {
      id: "opencode-bailian",
      providerId: "opencode",
      label: "Bailian",
      description: "Bailian OpenCode-compatible endpoint.",
      name: "Bailian",
      profileName: "bailian",
      baseUrl: "https://dashscope.aliyuncs.com/compatible-mode/v1",
      docsUrl: "https://bailian.console.aliyun.com",
    },
    {
      id: "opencode-kimi-k25",
      providerId: "opencode",
      label: "Kimi k2.5",
      description: "Kimi k2.5 OpenCode-compatible endpoint.",
      name: "Kimi k2.5",
      profileName: "kimi-k25",
      baseUrl: "https://api.moonshot.cn/v1",
      docsUrl: "https://platform.moonshot.cn/console",
    },
    {
      id: "opencode-kimi-coding",
      providerId: "opencode",
      label: "Kimi For Coding",
      description: "Kimi For Coding endpoint for OpenCode.",
      name: "Kimi For Coding",
      profileName: "kimi-coding",
      baseUrl: "https://api.kimi.com/v1",
      docsUrl: "https://www.kimi.com/coding/docs/",
    },
    {
      id: "opencode-kat-coder",
      providerId: "opencode",
      label: "KAT-Coder",
      description: "KAT-Coder gateway template; replace ENDPOINT_ID.",
      name: "KAT-Coder",
      profileName: "kat-coder",
      baseUrl: "https://vanchin.streamlake.ai/api/gateway/v1/endpoints/${ENDPOINT_ID}/openai",
      docsUrl: "https://console.streamlake.ai",
    },
    {
      id: "opencode-longcat",
      providerId: "opencode",
      label: "Longcat",
      description: "Longcat OpenCode-compatible endpoint.",
      name: "Longcat",
      profileName: "longcat",
      baseUrl: "https://api.longcat.chat/v1",
      docsUrl: "https://longcat.chat/platform",
    },
    {
      id: "opencode-minimax-cn",
      providerId: "opencode",
      label: "MiniMax",
      description: "MiniMax CN OpenCode-compatible endpoint.",
      name: "MiniMax",
      profileName: "minimax",
      baseUrl: "https://api.minimaxi.com/v1",
      docsUrl: "https://platform.minimaxi.com",
    },
    {
      id: "opencode-minimax-en",
      providerId: "opencode",
      label: "MiniMax en",
      description: "MiniMax EN OpenCode-compatible endpoint.",
      name: "MiniMax en",
      profileName: "minimax-en",
      baseUrl: "https://api.minimax.io/v1",
      docsUrl: "https://platform.minimax.io",
    },
    {
      id: "opencode-doubao-seed",
      providerId: "opencode",
      label: "DouBaoSeed",
      description: "DouBaoSeed OpenCode-compatible endpoint.",
      name: "DouBaoSeed",
      profileName: "doubao-seed",
      baseUrl: "https://ark.cn-beijing.volces.com/api/v3",
      docsUrl: "https://www.volcengine.com/product/doubao",
    },
    {
      id: "opencode-bailing",
      providerId: "opencode",
      label: "BaiLing",
      description: "BaiLing OpenCode-compatible endpoint.",
      name: "BaiLing",
      profileName: "bailing",
      baseUrl: "https://api.tbox.cn/v1",
      docsUrl: "https://alipaytbox.yuque.com/sxs0ba/ling/get_started",
    },
    {
      id: "opencode-xiaomi-mimo",
      providerId: "opencode",
      label: "Xiaomi MiMo",
      description: "Xiaomi MiMo OpenCode-compatible endpoint.",
      name: "Xiaomi MiMo",
      profileName: "xiaomi-mimo",
      baseUrl: "https://api.xiaomimimo.com/v1",
      docsUrl: "https://platform.xiaomimimo.com",
    },
    {
      id: "opencode-modelscope",
      providerId: "opencode",
      label: "ModelScope",
      description: "ModelScope OpenCode-compatible endpoint.",
      name: "ModelScope",
      profileName: "modelscope",
      baseUrl: "https://api-inference.modelscope.cn/v1",
      docsUrl: "https://modelscope.cn",
    },
    {
      id: "opencode-aihubmix",
      providerId: "opencode",
      label: "AiHubMix",
      description: "AiHubMix OpenCode-compatible endpoint.",
      name: "AiHubMix",
      profileName: "aihubmix",
      baseUrl: "https://aihubmix.com/v1",
      docsUrl: "https://aihubmix.com",
    },
    {
      id: "opencode-dmxapi",
      providerId: "opencode",
      label: "DMXAPI",
      description: "DMXAPI OpenCode-compatible endpoint.",
      name: "DMXAPI",
      profileName: "dmxapi",
      baseUrl: "https://www.dmxapi.cn/v1",
      docsUrl: "https://www.dmxapi.cn",
    },
    {
      id: "opencode-openrouter",
      providerId: "opencode",
      label: "OpenRouter",
      description: "OpenRouter OpenCode-compatible endpoint.",
      name: "OpenRouter",
      profileName: "openrouter",
      baseUrl: "https://openrouter.ai/api/v1",
      docsUrl: "https://openrouter.ai",
    },
    {
      id: "opencode-nvidia",
      providerId: "opencode",
      label: "Nvidia",
      description: "NVIDIA NIM OpenCode-compatible endpoint.",
      name: "Nvidia",
      profileName: "nvidia",
      baseUrl: "https://integrate.api.nvidia.com/v1",
      docsUrl: "https://build.nvidia.com",
    },
    {
      id: "opencode-packycode",
      providerId: "opencode",
      label: "PackyCode",
      description: "PackyCode OpenCode-compatible endpoint.",
      name: "PackyCode",
      profileName: "packycode",
      baseUrl: "https://www.packyapi.com/v1",
      docsUrl: "https://www.packyapi.com",
    },
    {
      id: "opencode-cubence",
      providerId: "opencode",
      label: "Cubence",
      description: "Cubence OpenCode-compatible endpoint.",
      name: "Cubence",
      profileName: "cubence",
      baseUrl: "https://api.cubence.com/v1",
      docsUrl: "https://cubence.com",
    },
    {
      id: "opencode-aigocode",
      providerId: "opencode",
      label: "AIGoCode",
      description: "AIGoCode OpenCode-compatible endpoint.",
      name: "AIGoCode",
      profileName: "aigocode",
      baseUrl: "https://api.aigocode.com",
      docsUrl: "https://aigocode.com",
    },
    {
      id: "opencode-rightcode",
      providerId: "opencode",
      label: "RightCode",
      description: "RightCode OpenCode-compatible endpoint.",
      name: "RightCode",
      profileName: "rightcode",
      baseUrl: "https://right.codes/codex/v1",
      opencodeNpm: "@ai-sdk/openai",
      docsUrl: "https://www.right.codes",
    },
    {
      id: "opencode-aicodemirror",
      providerId: "opencode",
      label: "AICodeMirror",
      description: "AICodeMirror OpenCode-compatible endpoint.",
      name: "AICodeMirror",
      profileName: "aicodemirror",
      baseUrl: "https://api.aicodemirror.com/api/claudecode",
      opencodeNpm: "@ai-sdk/anthropic",
      docsUrl: "https://www.aicodemirror.com",
    },
    {
      id: "opencode-aicoding",
      providerId: "opencode",
      label: "AICoding",
      description: "AICoding OpenCode-compatible endpoint.",
      name: "AICoding",
      profileName: "aicoding",
      baseUrl: "https://api.aicoding.sh",
      opencodeNpm: "@ai-sdk/anthropic",
      docsUrl: "https://www.aicoding.sh",
    },
    {
      id: "opencode-crazyrouter",
      providerId: "opencode",
      label: "CrazyRouter",
      description: "CrazyRouter OpenCode-compatible endpoint.",
      name: "CrazyRouter",
      profileName: "crazyrouter",
      baseUrl: "https://crazyrouter.com",
      opencodeNpm: "@ai-sdk/anthropic",
      docsUrl: "https://www.crazyrouter.com",
    },
    {
      id: "opencode-sssaicode",
      providerId: "opencode",
      label: "SSSAiCode",
      description: "SSSAiCode OpenCode-compatible endpoint.",
      name: "SSSAiCode",
      profileName: "sssaicode",
      baseUrl: "https://node-hk.sssaicode.com/api",
      opencodeNpm: "@ai-sdk/anthropic",
      docsUrl: "https://www.sssaicode.com",
    },
    {
      id: "opencode-aws-bedrock",
      providerId: "opencode",
      label: "AWS Bedrock",
      description: "Use AWS Bedrock credentials and region in config JSON.",
      name: "AWS Bedrock",
      note: "Fill AWS region and credentials in config JSON.",
      profileName: "aws-bedrock",
      configProtocol: "opencode_settings",
      opencodeNpm: "@ai-sdk/amazon-bedrock",
      docsUrl: "https://aws.amazon.com/bedrock/",
      configJson: JSON.stringify(
        {
          npm: "@ai-sdk/amazon-bedrock",
          name: "AWS Bedrock",
          options: {
            region: "us-west-2",
            accessKeyId: "",
            secretAccessKey: "",
          },
          models: {},
        },
        null,
        2,
      ),
    },
    {
      id: "opencode-openai-compatible",
      providerId: "opencode",
      label: "OpenAI Compatible",
      description: "Custom OpenAI-compatible template for OpenCode.",
      name: "OpenAI Compatible",
      note: "Template preset: fill base URL and API key manually.",
      profileName: "openai-compatible",
    },
    {
      id: "opencode-oh-my-opencode",
      providerId: "opencode",
      label: "Oh My OpenCode",
      description: "Community toolkit for OpenCode.",
      name: "Oh My OpenCode",
      profileName: "oh-my-opencode",
      configProtocol: "opencode_settings",
      docsUrl: "https://github.com/code-yeongyu/oh-my-opencode",
      configJson: JSON.stringify(
        {
          npm: "",
          options: {},
          models: {},
        },
        null,
        2,
      ),
    },
    {
      id: "opencode-oh-my-opencode-slim",
      providerId: "opencode",
      label: "Oh My OpenCode Slim",
      description: "Lightweight community toolkit for OpenCode.",
      name: "Oh My OpenCode Slim",
      profileName: "oh-my-opencode-slim",
      configProtocol: "opencode_settings",
      docsUrl: "https://github.com/alvinunreal/oh-my-opencode-slim",
      configJson: JSON.stringify(
        {
          npm: "",
          options: {},
          models: {},
        },
        null,
        2,
      ),
    },
  ],
};

const CODEX_MODEL_BY_PRESET_ID: Record<string, string> = {
  "codex-aicoding": "gpt-5.3-codex",
  "codex-crazyrouter": "gpt-5.3-codex",
  "codex-sssaicode": "gpt-5.3-codex",
};

function sanitizeCodexProviderKey(raw: string): string {
  const normalized = raw
    .toLowerCase()
    .replace(/[^a-z0-9_]+/g, "_")
    .replace(/^_+|_+$/g, "");
  return normalized.length > 0 ? normalized : "custom";
}

function buildCodexConfigToml(
  providerKey: string,
  providerName: string,
  baseUrl: string,
  model: string,
): string {
  return `model_provider = "${providerKey}"
model = "${model}"
model_reasoning_effort = "high"
disable_response_storage = true

[model_providers.${providerKey}]
name = "${providerName}"
base_url = "${baseUrl}"
wire_api = "responses"
requires_openai_auth = true`;
}

function buildCodexConfigObject(
  providerKey: string,
  providerName: string,
  baseUrl: string,
  model: string,
): Record<string, unknown> {
  return {
    auth: {
      OPENAI_API_KEY: "",
    },
    config: buildCodexConfigToml(providerKey, providerName, baseUrl, model),
  };
}

function buildCodexPresetConfig(preset: SupplierPreset): string {
  if (preset.id === "codex-openai-official") {
    return JSON.stringify(
      {
        auth: {},
        config: "",
      },
      null,
      2,
    );
  }

  if (preset.id === "codex-azure-openai") {
    const config = `model_provider = "azure"
model = "gpt-5.2"
model_reasoning_effort = "high"
disable_response_storage = true

[model_providers.azure]
name = "Azure OpenAI"
base_url = "https://YOUR_RESOURCE_NAME.openai.azure.com/openai"
env_key = "OPENAI_API_KEY"
query_params = { "api-version" = "2025-04-01-preview" }
wire_api = "responses"
requires_openai_auth = true`;
    return JSON.stringify(
      {
        auth: {
          OPENAI_API_KEY: "",
        },
        config,
      },
      null,
      2,
    );
  }

  const baseUrl = preset.baseUrl ?? "https://api.openai.com/v1";
  const providerKey = sanitizeCodexProviderKey(preset.profileName || preset.name);
  const model = CODEX_MODEL_BY_PRESET_ID[preset.id] ?? "gpt-5.2";
  return JSON.stringify(
    buildCodexConfigObject(providerKey, preset.name, baseUrl, model),
    null,
    2,
  );
}

function buildOpenCodeConfigObject(
  name: string,
  baseUrl: string,
  npm: string,
): Record<string, unknown> {
  return {
    npm,
    name,
    options: {
      baseURL: baseUrl,
      apiKey: "",
    },
    models: {},
  };
}

function resolveOpenCodePresetNpm(preset: SupplierPreset): string {
  if (preset.opencodeNpm) {
    return preset.opencodeNpm;
  }
  return "@ai-sdk/openai-compatible";
}

function buildOpenCodePresetConfig(preset: SupplierPreset): string {
  if (preset.configJson) {
    return preset.configJson;
  }

  const npm = resolveOpenCodePresetNpm(preset);
  if (npm.length === 0) {
    return JSON.stringify(
      {
        npm: "",
        options: {},
        models: {},
      },
      null,
      2,
    );
  }

  const baseUrl = preset.baseUrl ?? "";
  return JSON.stringify(
    buildOpenCodeConfigObject(preset.name, baseUrl, npm),
    null,
    2,
  );
}

function configJsonForPreset(preset: SupplierPreset): string | undefined {
  if (preset.id === "manual-custom-empty") {
    return preset.configJson;
  }
  if (preset.providerId === "codex") {
    return buildCodexPresetConfig(preset);
  }
  if (preset.providerId === "opencode") {
    return buildOpenCodePresetConfig(preset);
  }
  return preset.configJson;
}

const CONFIG_TEMPLATE: Record<ThreadProviderId, Record<string, unknown>> = {
  claude_code: {
    env: {
      API_TIMEOUT_MS: "300000",
    },
  },
  codex: buildCodexConfigObject(
    "custom",
    "custom",
    "https://api.openai.com/v1",
    "gpt-5.3-codex",
  ),
  opencode: buildOpenCodeConfigObject(
    "OpenAI Compatible",
    "https://api.openai.com/v1",
    "@ai-sdk/openai-compatible",
  ),
};

function emptyProviderInstallStatusMap(): Record<ThreadProviderId, ProviderInstallStatus | null> {
  return {
    claude_code: null,
    codex: null,
    opencode: null,
  };
}

function resolvePickedDirectory(
  picked: string | string[] | null,
): string | null {
  if (typeof picked === "string") {
    return picked;
  }
  if (Array.isArray(picked)) {
    const first = picked[0];
    return typeof first === "string" ? first : null;
  }
  return null;
}

function sanitizeProjectPath(path: string): string {
  const normalized = normalizeProjectPath(path);
  return normalized === "." ? "" : normalized;
}

function isDarkModeTheme(theme: AppTheme): boolean {
  if (theme === "dark") {
    return true;
  }
  if (theme === "light") {
    return false;
  }
  if (typeof window === "undefined") {
    return false;
  }
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

function providerStatusLabel(
  status: ProviderInstallStatus | null,
  loading: boolean,
): string {
  if (loading && !status) {
    return "Checking...";
  }
  if (!status) {
    return "Unknown";
  }
  if (!status.installed) {
    return "Not Installed";
  }
  return status.healthStatus === "degraded" ? "Installed (Setup Needed)" : "Installed";
}

function providerStatusClass(
  status: ProviderInstallStatus | null,
  loading: boolean,
): string {
  if (loading && !status) {
    return "text-muted-foreground";
  }
  if (!status || !status.installed) {
    return "text-destructive";
  }
  if (status.healthStatus === "degraded") {
    return "text-amber-600 dark:text-amber-400";
  }
  return "text-emerald-600 dark:text-emerald-400";
}

function cloneAgentRuntimeSettings(settings: AgentRuntimeSettings): AgentRuntimeSettings {
  return JSON.parse(JSON.stringify(settings)) as AgentRuntimeSettings;
}

function normalizeOptionalText(value: string | null | undefined): string | undefined {
  const trimmed = value?.trim();
  return trimmed && trimmed.length > 0 ? trimmed : undefined;
}

function normalizeProfileName(value: string | null | undefined): string {
  return normalizeOptionalText(value) ?? "default";
}

function sanitizeSupplierIdSegment(raw: string): string {
  const normalized = raw
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
  return normalized || "imported";
}

function importedSupplierId(providerId: ThreadProviderId, sourceId: string): string {
  return `custom-ccswitch-${providerId}-${sanitizeSupplierIdSegment(sourceId)}`;
}

function nextSupplierName(baseName: string, suppliers: AgentSupplier[]): string {
  const usedNames = new Set(suppliers.map((supplier) => supplier.name.trim().toLowerCase()));
  if (!usedNames.has(baseName.trim().toLowerCase())) {
    return baseName;
  }

  let index = 2;
  while (usedNames.has(`${baseName} ${index}`.toLowerCase())) {
    index += 1;
  }
  return `${baseName} ${index}`;
}

function createPresetSupplier(
  preset: SupplierPreset,
  suppliers: AgentSupplier[],
): AgentSupplier {
  const now = Date.now();
  return {
    id: `custom-${preset.providerId}-${preset.id}-${now}`,
    kind: "custom",
    name: nextSupplierName(preset.name, suppliers),
    note: preset.note ?? preset.description,
    profileName: preset.profileName,
    baseUrl: preset.baseUrl,
    apiKey: undefined,
    configJson: configJsonForPreset(preset),
    updatedAt: now,
  };
}

function configTemplateForProvider(providerId: ThreadProviderId): string {
  return JSON.stringify(CONFIG_TEMPLATE[providerId], null, 2);
}

function configProtocolHint(providerId: ThreadProviderId): string {
  if (providerId === "codex") {
    return "Config JSON (optional, preferred: `auth` + `config`)";
  }
  if (providerId === "opencode") {
    return "Config JSON (optional, preferred: `npm/options/models`)";
  }
  return "Config JSON (optional, preferred: `env` object)";
}

function firstPresetId(providerId: ThreadProviderId): string {
  return SUPPLIER_PRESETS[providerId][0]?.id ?? "";
}

function jsonValidationErrorMessage(raw: string, error: unknown): string {
  const baseMessage = error instanceof Error ? error.message : String(error);
  const positionMatch = /position (\d+)/i.exec(baseMessage);
  if (!positionMatch) {
    return baseMessage;
  }

  const position = Number(positionMatch[1]);
  if (!Number.isFinite(position) || position < 0 || position > raw.length) {
    return baseMessage;
  }

  let line = 1;
  let column = 1;
  for (let index = 0; index < position; index += 1) {
    if (raw[index] === "\n") {
      line += 1;
      column = 1;
    } else {
      column += 1;
    }
  }

  return `${baseMessage} (line ${line}, column ${column})`;
}

function asJsonRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function validateConfigJson(
  providerId: ThreadProviderId,
  raw: string | undefined,
): JsonValidationResult {
  const value = raw?.trim() ?? "";
  if (!value) {
    return {
      isValid: true,
      message: "Empty: no protocol overrides.",
    };
  }

  try {
    const parsed = JSON.parse(value) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return {
        isValid: false,
        message: "Config JSON must be an object.",
      };
    }

    const parsedRecord = parsed as Record<string, unknown>;
    if (
      parsedRecord.env !== undefined &&
      (typeof parsedRecord.env !== "object"
        || parsedRecord.env === null
        || Array.isArray(parsedRecord.env))
    ) {
      return {
        isValid: false,
        message: "Config JSON field `env` must be an object.",
      };
    }

    if (providerId === "codex") {
      const auth = asJsonRecord(parsedRecord.auth);
      const hasCodexConfig =
        auth !== null
        || typeof parsedRecord.config === "string"
        || asJsonRecord(parsedRecord.config) !== null;
      return {
        isValid: true,
        message: hasCodexConfig
          ? "JSON is valid (Codex auth/config protocol detected)."
          : "JSON is valid. Recommended Codex shape: { auth, config }.",
      };
    }

    if (providerId === "opencode") {
      const opencodeConfig = asJsonRecord(parsedRecord.settingsConfig) ?? parsedRecord;
      const hasOpenCodeShape =
        typeof opencodeConfig.npm === "string"
        && asJsonRecord(opencodeConfig.options) !== null
        && asJsonRecord(opencodeConfig.models) !== null;
      return {
        isValid: true,
        message: hasOpenCodeShape
          ? "JSON is valid (OpenCode settings protocol detected)."
          : "JSON is valid. Recommended OpenCode shape: { npm, options, models }.",
      };
    }

    return {
      isValid: true,
      message: "JSON is valid. Recommended Claude shape: { env: { ... } }.",
    };
  } catch (error) {
    return {
      isValid: false,
      message: jsonValidationErrorMessage(value, error),
    };
  }
}

function resolveProviderActiveProfile(
  settings: AgentRuntimeSettings,
  providerId: ThreadProviderId,
): string {
  const suppliers = settings.suppliersByProvider[providerId] ?? [];
  const activeSupplierId = settings.activeSupplierIds[providerId];
  const activeSupplier = suppliers.find((supplier) => supplier.id === activeSupplierId);
  return activeSupplier?.profileName ?? "default";
}

export interface SidebarProps {
  sidebarCollapsed: boolean;
  folderGroups: ThreadFolderGroupItem[];
  selectedFolderKey: string | null;
  selectedThreadKey: string | null;
  loadingThreads: boolean;
  creatingThreadFolderKey: string | null;
  error: string | null;
  newThreadBindingStatus: "starting" | "awaiting_discovery" | null;
  hasPendingNewThreadLaunch: boolean;
  appTheme: AppTheme;
  activeProviderId: ThreadProviderId;
  activeProfileName: string;
  providerProfiles: ProviderProfileMap;
  agentRuntimeSettings: AgentRuntimeSettings;
  onLoadThreads: () => void;
  onSelectThread: (threadKey: string) => void;
  onCreateThread: (projectPath: string, providerId: ThreadProviderId) => Promise<void>;
  onAgentRuntimeSettingsChange: (selection: AgentRuntimeSettings) => string | null;
  onAppThemeChange: (theme: AppTheme) => void;
  onClearError: () => void;
}

export function Sidebar({
  sidebarCollapsed,
  folderGroups,
  selectedFolderKey,
  selectedThreadKey,
  loadingThreads,
  creatingThreadFolderKey,
  error,
  newThreadBindingStatus,
  hasPendingNewThreadLaunch,
  appTheme,
  activeProviderId,
  activeProfileName,
  providerProfiles,
  agentRuntimeSettings,
  onLoadThreads,
  onSelectThread,
  onCreateThread,
  onAgentRuntimeSettingsChange,
  onAppThemeChange,
  onClearError,
}: SidebarProps) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [themeDialogOpen, setThemeDialogOpen] = useState(false);
  const [accountDialogOpen, setAccountDialogOpen] = useState(false);
  const [skillsDialogOpen, setSkillsDialogOpen] = useState(false);
  const [pendingTheme, setPendingTheme] = useState<AppTheme>(appTheme);
  const [pendingActiveProviderId, setPendingActiveProviderId] =
    useState<ThreadProviderId>(activeProviderId);
  const [pendingRuntimeSettings, setPendingRuntimeSettings] =
    useState<AgentRuntimeSettings>(cloneAgentRuntimeSettings(agentRuntimeSettings));
  const [editingSupplierId, setEditingSupplierId] = useState<string | null>(OFFICIAL_SUPPLIER_ID);
  const [selectedPresetId, setSelectedPresetId] = useState("");
  const [configEditorNotice, setConfigEditorNotice] =
    useState<ConfigEditorNotice | null>(null);
  const [accountDialogError, setAccountDialogError] = useState<string | null>(null);
  const [ccSwitchImportNotice, setCcSwitchImportNotice] = useState<string | null>(null);
  const [ccSwitchImporting, setCcSwitchImporting] = useState(false);

  const [newThreadDialogOpen, setNewThreadDialogOpen] = useState(false);
  const [selectedProjectPath, setSelectedProjectPath] = useState("");
  const [selectedProviderId, setSelectedProviderId] =
    useState<ThreadProviderId>("claude_code");
  const [isPickingFolder, setIsPickingFolder] = useState(false);
  const [didAttemptCreate, setDidAttemptCreate] = useState(false);
  const [createRequested, setCreateRequested] = useState(false);
  const [didObserveLaunchState, setDidObserveLaunchState] = useState(false);
  const [pickerError, setPickerError] = useState<string | null>(null);
  const [createDialogError, setCreateDialogError] = useState<string | null>(null);
  const [providerInstallStatuses, setProviderInstallStatuses] = useState<
    Record<ThreadProviderId, ProviderInstallStatus | null>
  >(emptyProviderInstallStatusMap);
  const [providerStatusLoading, setProviderStatusLoading] = useState(false);
  const [providerStatusError, setProviderStatusError] = useState<string | null>(null);
  const [providerInstallGuideError, setProviderInstallGuideError] = useState<string | null>(null);

  const settingsRef = useRef<HTMLDivElement | null>(null);

  const folderKeys = useMemo(
    () => new Set(folderGroups.map((group) => group.key)),
    [folderGroups],
  );

  const hasLaunchInFlight = hasPendingNewThreadLaunch || newThreadBindingStatus !== null;
  const isCreateBusy = isPickingFolder || createRequested || hasLaunchInFlight;

  const selectedPathValue = sanitizeProjectPath(selectedProjectPath);
  const hasSelectedFolderInList = folderKeys.has(selectedPathValue);
  const selectedProviderInstallStatus = providerInstallStatuses[selectedProviderId];
  const selectedProviderInstalled = selectedProviderInstallStatus?.installed ?? true;
  const selectedProviderStatusResolved =
    selectedProviderInstallStatus !== null || !providerStatusLoading;
  const canCreate =
    selectedPathValue.length > 0 &&
    !isCreateBusy &&
    selectedProviderStatusResolved &&
    selectedProviderInstalled;

  const createStatusText =
    newThreadBindingStatus === "starting"
      ? "Starting terminal session..."
      : newThreadBindingStatus === "awaiting_discovery"
        ? "Session started. Waiting for first input to persist thread id..."
        : null;

  const visibleCreateError =
    createDialogError ?? (didAttemptCreate ? error : null);

  const loadProviderInstallStatuses = useCallback(
    async (projectPath: string) => {
      setProviderStatusLoading(true);
      setProviderStatusError(null);

      try {
        const normalizedProjectPath = sanitizeProjectPath(projectPath);
        const data = await invoke<ProviderInstallStatusPayload[]>(
          "list_provider_install_statuses",
          {
            projectPath: normalizedProjectPath.length > 0 ? normalizedProjectPath : null,
          },
        );

        const nextStatuses = emptyProviderInstallStatusMap();
        for (const item of data) {
          if (!isSupportedProvider(item.providerId)) {
            continue;
          }
          nextStatuses[item.providerId] = {
            providerId: item.providerId,
            installed: item.installed,
            healthStatus: item.healthStatus,
            message: item.message,
          };
        }
        setProviderInstallStatuses(nextStatuses);
      } catch (statusError) {
        const message =
          statusError instanceof Error ? statusError.message : String(statusError);
        setProviderStatusError(message);
      } finally {
        setProviderStatusLoading(false);
      }
    },
    [],
  );

  const handleOpenInstallGuide = useCallback(async () => {
    setProviderInstallGuideError(null);
    try {
      await openUrl(providerInstallGuideUrl(selectedProviderId));
    } catch (openError) {
      const message =
        openError instanceof Error ? openError.message : String(openError);
      setProviderInstallGuideError(message);
    }
  }, [selectedProviderId]);

  useEffect(() => {
    if (!settingsOpen && !themeDialogOpen && !accountDialogOpen && !newThreadDialogOpen) {
      return;
    }

    const handlePointerDown = (event: PointerEvent) => {
      if (themeDialogOpen || accountDialogOpen || newThreadDialogOpen) {
        return;
      }
      if (!settingsOpen) {
        return;
      }
      const target = event.target;
      if (!(target instanceof Node)) {
        return;
      }
      if (!settingsRef.current?.contains(target)) {
        setSettingsOpen(false);
      }
    };

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      if (themeDialogOpen) {
        setThemeDialogOpen(false);
        return;
      }
      if (accountDialogOpen) {
        setAccountDialogOpen(false);
        return;
      }
      if (newThreadDialogOpen) {
        if (!isCreateBusy) {
          setNewThreadDialogOpen(false);
        }
        return;
      }
      setSettingsOpen(false);
    };

    window.addEventListener("pointerdown", handlePointerDown);
    window.addEventListener("keydown", handleEscape);
    return () => {
      window.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("keydown", handleEscape);
    };
  }, [
    settingsOpen,
    themeDialogOpen,
    accountDialogOpen,
    newThreadDialogOpen,
    isCreateBusy,
  ]);

  useEffect(() => {
    if (sidebarCollapsed) {
      setSettingsOpen(false);
      setThemeDialogOpen(false);
      setAccountDialogOpen(false);
      setNewThreadDialogOpen(false);
    }
  }, [sidebarCollapsed]);

  useEffect(() => {
    if (!themeDialogOpen) {
      setPendingTheme(appTheme);
    }
  }, [appTheme, themeDialogOpen]);

  useEffect(() => {
    if (!accountDialogOpen) {
      setPendingActiveProviderId(activeProviderId);
      setPendingRuntimeSettings(cloneAgentRuntimeSettings(agentRuntimeSettings));
      setEditingSupplierId(agentRuntimeSettings.activeSupplierIds[activeProviderId] ?? OFFICIAL_SUPPLIER_ID);
      setSelectedPresetId(firstPresetId(activeProviderId));
      setConfigEditorNotice(null);
      setAccountDialogError(null);
      setCcSwitchImportNotice(null);
      setCcSwitchImporting(false);
    }
  }, [accountDialogOpen, activeProviderId, agentRuntimeSettings]);

  useEffect(() => {
    if (!accountDialogOpen) {
      return;
    }

    const presets = SUPPLIER_PRESETS[pendingActiveProviderId] ?? [];
    if (presets.length === 0) {
      if (selectedPresetId !== "") {
        setSelectedPresetId("");
      }
      return;
    }

    const hasSelectedPreset = presets.some((preset) => preset.id === selectedPresetId);
    if (!hasSelectedPreset) {
      setSelectedPresetId(presets[0].id);
    }
  }, [accountDialogOpen, pendingActiveProviderId, selectedPresetId]);

  useEffect(() => {
    if (!accountDialogOpen) {
      return;
    }
    setConfigEditorNotice(null);
  }, [accountDialogOpen, pendingActiveProviderId, editingSupplierId]);

  useEffect(() => {
    if (!newThreadDialogOpen || !createRequested) {
      return;
    }

    if (hasLaunchInFlight) {
      if (!didObserveLaunchState) {
        setDidObserveLaunchState(true);
      }
      if (newThreadBindingStatus === "awaiting_discovery") {
        setCreateRequested(false);
        setNewThreadDialogOpen(false);
      }
      return;
    }

    if (!didObserveLaunchState) {
      return;
    }

    if (visibleCreateError) {
      setCreateRequested(false);
      return;
    }

    setCreateRequested(false);
    setNewThreadDialogOpen(false);
  }, [
    newThreadDialogOpen,
    createRequested,
    newThreadBindingStatus,
    hasLaunchInFlight,
    didObserveLaunchState,
    visibleCreateError,
  ]);

  const selectedThemeOption = APP_THEME_OPTIONS.find(
    (option) => option.value === appTheme,
  );
  const pendingThemeOption = APP_THEME_OPTIONS.find(
    (option) => option.value === pendingTheme,
  );
  const activeProviderOption = THREAD_PROVIDER_OPTIONS.find(
    (option) => option.value === activeProviderId,
  );
  const pendingActiveProviderOption = THREAD_PROVIDER_OPTIONS.find(
    (option) => option.value === pendingActiveProviderId,
  );
  const pendingProviderSuppliers =
    pendingRuntimeSettings.suppliersByProvider[pendingActiveProviderId] ?? [];
  const pendingProviderPresets = SUPPLIER_PRESETS[pendingActiveProviderId] ?? [];
  const pendingActiveSupplierId =
    pendingRuntimeSettings.activeSupplierIds[pendingActiveProviderId] ?? OFFICIAL_SUPPLIER_ID;
  const editingSupplier =
    pendingProviderSuppliers.find((supplier) => supplier.id === editingSupplierId)
    ?? pendingProviderSuppliers.find((supplier) => supplier.id === pendingActiveSupplierId)
    ?? null;
  const selectedPreset =
    pendingProviderPresets.find((preset) => preset.id === selectedPresetId)
    ?? pendingProviderPresets[0]
    ?? null;
  const editingOfficialSupplier = editingSupplier?.kind === "official";
  const editingConfigValidation = validateConfigJson(
    pendingActiveProviderId,
    editingSupplier?.configJson,
  );
  const pendingActiveProfileName = resolveProviderActiveProfile(
    pendingRuntimeSettings,
    pendingActiveProviderId,
  );
  const configEditorDarkMode = isDarkModeTheme(appTheme);

  const openThemeDialog = () => {
    setPendingTheme(appTheme);
    setThemeDialogOpen(true);
    setSettingsOpen(false);
  };

  const openAccountDialog = () => {
    setPendingActiveProviderId(activeProviderId);
    setPendingRuntimeSettings(cloneAgentRuntimeSettings(agentRuntimeSettings));
    setEditingSupplierId(agentRuntimeSettings.activeSupplierIds[activeProviderId] ?? OFFICIAL_SUPPLIER_ID);
    setSelectedPresetId(firstPresetId(activeProviderId));
    setConfigEditorNotice(null);
    setAccountDialogError(null);
    setCcSwitchImportNotice(null);
    setCcSwitchImporting(false);
    setAccountDialogOpen(true);
    setSettingsOpen(false);
  };

  const applyThemeChange = () => {
    onAppThemeChange(pendingTheme);
    setThemeDialogOpen(false);
  };

  const applyActiveAgentProfileChange = () => {
    const message = onAgentRuntimeSettingsChange(pendingRuntimeSettings);
    if (message) {
      setAccountDialogError(message);
      return;
    }
    setAccountDialogError(null);
    setAccountDialogOpen(false);
  };

  const setPendingActiveSupplier = (
    providerId: ThreadProviderId,
    supplierId: string,
  ) => {
    setPendingRuntimeSettings((current) => ({
      ...current,
      activeProviderId: providerId,
      activeSupplierIds: {
        ...current.activeSupplierIds,
        [providerId]: supplierId,
      },
    }));
    setPendingActiveProviderId(providerId);
    setEditingSupplierId(supplierId);
    setConfigEditorNotice(null);
    setAccountDialogError(null);
  };

  const updatePendingSupplier = (
    providerId: ThreadProviderId,
    supplierId: string,
    patch: Partial<AgentSupplier>,
  ) => {
    setPendingRuntimeSettings((current) => {
      const suppliers = current.suppliersByProvider[providerId] ?? [];
      const nextSuppliers = suppliers.map((supplier) =>
        supplier.id === supplierId
          ? { ...supplier, ...patch, updatedAt: Date.now() }
          : supplier,
      );
      return {
        ...current,
        suppliersByProvider: {
          ...current.suppliersByProvider,
          [providerId]: nextSuppliers,
        },
      };
    });
    setConfigEditorNotice(null);
    setAccountDialogError(null);
  };

  const addPendingPresetSupplier = () => {
    if (!selectedPreset || selectedPreset.providerId !== pendingActiveProviderId) {
      return;
    }

    const presetSupplier = createPresetSupplier(selectedPreset, pendingProviderSuppliers);
    setPendingRuntimeSettings((current) => ({
      ...current,
      suppliersByProvider: {
        ...current.suppliersByProvider,
        [pendingActiveProviderId]: [
          ...(current.suppliersByProvider[pendingActiveProviderId] ?? []),
          presetSupplier,
        ],
      },
    }));
    setEditingSupplierId(presetSupplier.id);
    setConfigEditorNotice({
      tone: "success",
      message: `Preset "${selectedPreset.label}" added.`,
    });
    setAccountDialogError(null);
  };

  const importCcSwitchSuppliers = async () => {
    if (ccSwitchImporting) {
      return;
    }

    setCcSwitchImporting(true);
    setCcSwitchImportNotice(null);
    setAccountDialogError(null);
    setConfigEditorNotice(null);

    try {
      const payload = await invoke<CcSwitchImportPayload>("import_ccswitch_suppliers");
      const importedSuppliers = payload.suppliers.filter(
        (
          supplier,
        ): supplier is CcSwitchImportedSupplierPayload & { providerId: ThreadProviderId } =>
          isSupportedProvider(supplier.providerId),
      );

      if (importedSuppliers.length === 0) {
        setCcSwitchImportNotice(
          `No Claude/Codex suppliers were found in ${payload.dbPath}.`,
        );
        return;
      }

      const nextSettings = cloneAgentRuntimeSettings(pendingRuntimeSettings);
      let addedCount = 0;
      let updatedCount = 0;
      let firstImportedSelection: { providerId: ThreadProviderId; supplierId: string } | null =
        null;

      for (const imported of importedSuppliers) {
        const providerId = imported.providerId;
        const supplierId = importedSupplierId(providerId, imported.sourceId);
        const suppliers = [...(nextSettings.suppliersByProvider[providerId] ?? [])];
        const now = Date.now();
        const importedName =
          normalizeOptionalText(imported.name)
          ?? nextSupplierName("Imported Supplier", suppliers);
        const nextSupplier: AgentSupplier = {
          id: supplierId,
          kind: "custom",
          name: importedName,
          note: normalizeOptionalText(imported.note) ?? "Imported from CC Switch",
          profileName: normalizeProfileName(imported.profileName),
          baseUrl: normalizeOptionalText(imported.baseUrl),
          apiKey: normalizeOptionalText(imported.apiKey),
          configJson: normalizeOptionalText(imported.configJson),
          updatedAt: now,
        };

        const existingIndex = suppliers.findIndex((supplier) => supplier.id === supplierId);
        if (existingIndex >= 0) {
          suppliers[existingIndex] = nextSupplier;
          updatedCount += 1;
        } else {
          suppliers.push(nextSupplier);
          addedCount += 1;
        }

        nextSettings.suppliersByProvider[providerId] = suppliers;
        if (!firstImportedSelection || imported.isCurrent) {
          firstImportedSelection = { providerId, supplierId };
        }
        if (imported.isCurrent) {
          nextSettings.activeSupplierIds[providerId] = supplierId;
          nextSettings.activeProviderId = providerId;
        }
      }

      setPendingRuntimeSettings(nextSettings);
      if (firstImportedSelection) {
        setPendingActiveProviderId(firstImportedSelection.providerId);
        setEditingSupplierId(firstImportedSelection.supplierId);
        setSelectedPresetId(firstPresetId(firstImportedSelection.providerId));
      }

      const totalCount = addedCount + updatedCount;
      setCcSwitchImportNotice(
        `Imported ${totalCount} suppliers from ${payload.dbPath} (${addedCount} added, ${updatedCount} updated).`,
      );
    } catch (importError) {
      const message =
        importError instanceof Error ? importError.message : String(importError);
      setAccountDialogError(message);
    } finally {
      setCcSwitchImporting(false);
    }
  };

  const openPresetDocs = async () => {
    if (!selectedPreset?.docsUrl) {
      return;
    }
    try {
      await openUrl(selectedPreset.docsUrl);
    } catch (openError) {
      const message =
        openError instanceof Error ? openError.message : String(openError);
      setAccountDialogError(message);
    }
  };

  const applyConfigTemplate = () => {
    if (!editingSupplier) {
      return;
    }
    updatePendingSupplier(pendingActiveProviderId, editingSupplier.id, {
      configJson: configTemplateForProvider(pendingActiveProviderId),
    });
    setConfigEditorNotice({
      tone: "success",
      message: "Applied template JSON.",
    });
  };

  const formatEditingConfigJson = () => {
    if (!editingSupplier) {
      return;
    }

    const source = editingSupplier.configJson?.trim() ?? "";
    if (!source) {
      setConfigEditorNotice({
        tone: "error",
        message: "Config JSON is empty. Insert a template first.",
      });
      return;
    }

    try {
      const parsed = JSON.parse(source) as unknown;
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
        setConfigEditorNotice({
          tone: "error",
          message: "Config JSON must be an object.",
        });
        return;
      }
      updatePendingSupplier(pendingActiveProviderId, editingSupplier.id, {
        configJson: JSON.stringify(parsed, null, 2),
      });
      setConfigEditorNotice({
        tone: "success",
        message: "Config JSON formatted.",
      });
    } catch (formatError) {
      setConfigEditorNotice({
        tone: "error",
        message: jsonValidationErrorMessage(source, formatError),
      });
    }
  };

  const validateEditingConfigJson = () => {
    if (!editingSupplier) {
      return;
    }
    const validation = validateConfigJson(
      pendingActiveProviderId,
      editingSupplier.configJson,
    );
    setConfigEditorNotice({
      tone: validation.isValid ? "success" : "error",
      message: validation.message,
    });
  };

  const clearEditingConfigJson = () => {
    if (!editingSupplier) {
      return;
    }
    updatePendingSupplier(pendingActiveProviderId, editingSupplier.id, {
      configJson: undefined,
    });
    setConfigEditorNotice({
      tone: "success",
      message: "Cleared config JSON.",
    });
  };

  const deletePendingSupplier = (
    providerId: ThreadProviderId,
    supplierId: string,
  ) => {
    setPendingRuntimeSettings((current) => {
      const suppliers = current.suppliersByProvider[providerId] ?? [];
      const nextSuppliers = suppliers.filter((supplier) => supplier.id !== supplierId);
      const hasActiveSupplier = nextSuppliers.some(
        (supplier) => supplier.id === current.activeSupplierIds[providerId],
      );
      return {
        ...current,
        activeSupplierIds: {
          ...current.activeSupplierIds,
          [providerId]: hasActiveSupplier
            ? current.activeSupplierIds[providerId]
            : OFFICIAL_SUPPLIER_ID,
        },
        suppliersByProvider: {
          ...current.suppliersByProvider,
          [providerId]: nextSuppliers,
        },
      };
    });
    setEditingSupplierId(OFFICIAL_SUPPLIER_ID);
    setConfigEditorNotice(null);
    setAccountDialogError(null);
  };

  const openNewThreadDialog = () => {
    const fallbackProjectPath =
      (selectedFolderKey && selectedFolderKey !== "." ? selectedFolderKey : null)
      ?? folderGroups[0]?.key
      ?? "";

    setSelectedProjectPath(fallbackProjectPath);
    setSelectedProviderId(activeProviderId);
    setIsPickingFolder(false);
    setDidAttemptCreate(false);
    setCreateRequested(false);
    setDidObserveLaunchState(false);
    setPickerError(null);
    setCreateDialogError(null);
    setProviderInstallStatuses(emptyProviderInstallStatusMap());
    setProviderStatusError(null);
    setProviderInstallGuideError(null);
    onClearError();
    setNewThreadDialogOpen(true);
    void loadProviderInstallStatuses(fallbackProjectPath);
  };

  const closeNewThreadDialog = () => {
    if (isCreateBusy) {
      return;
    }
    setNewThreadDialogOpen(false);
  };

  const handlePickFolder = async () => {
    setPickerError(null);
    setCreateDialogError(null);
    onClearError();
    setIsPickingFolder(true);

    try {
      const picked = await open({
        directory: true,
        multiple: false,
        title: "Choose project folder",
      });
      const path = resolvePickedDirectory(picked);
      if (!path) {
        return;
      }
      const sanitizedPath = sanitizeProjectPath(path);
      setSelectedProjectPath(sanitizedPath);
      void loadProviderInstallStatuses(sanitizedPath);
    } catch (pickError) {
      const message =
        pickError instanceof Error ? pickError.message : String(pickError);
      setPickerError(message);
    } finally {
      setIsPickingFolder(false);
    }
  };

  const handleCreateFromDialog = async () => {
    if (!canCreate) {
      return;
    }

    const projectPath = sanitizeProjectPath(selectedProjectPath);
    if (!projectPath) {
      return;
    }

    setDidAttemptCreate(true);
    setCreateRequested(true);
    setDidObserveLaunchState(false);
    setCreateDialogError(null);
    setPickerError(null);
    onClearError();

    try {
      await onCreateThread(projectPath, selectedProviderId);
    } catch (createError) {
      const message =
        createError instanceof Error ? createError.message : String(createError);
      setCreateDialogError(message);
      setCreateRequested(false);
    }
  };

  if (sidebarCollapsed) {
    return null;
  }

  return (
    <Card className="flex min-h-0 flex-col rounded-none border-0 bg-card/92 shadow-none pt-8">
      <CardHeader className="px-4 py-3 pb-2.5">
        <div className="flex items-center gap-2 pt-1">
          <Button
            variant="default"
            size="sm"
            className="h-7 px-2.5 text-xs font-semibold shadow-sm hover:shadow"
            onClick={openNewThreadDialog}
            disabled={isCreateBusy}
            aria-label="Create a new thread"
          >
            {isCreateBusy ? (
              <Loader2 className="mr-1.5 h-3 w-3 animate-spin" />
            ) : (
              <Plus className="mr-1.5 h-3 w-3" />
            )}
            {isCreateBusy ? "Creating..." : "New Thread"}
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-7 px-2.5 text-xs"
            onClick={() => void onLoadThreads()}
            disabled={loadingThreads}
          >
            <RefreshCw className="mr-1.5 h-3 w-3" />
            Refresh
          </Button>
        </div>
      </CardHeader>
      <CardContent className="flex min-h-0 flex-1 flex-col gap-2.5 overflow-hidden py-2.5 pl-2.5 pr-2.5">
        <div className="flex min-h-0 flex-1 flex-col">
          <div className="mb-1.5 flex items-center justify-between px-0.5 pr-2.5">
            <p className="text-xs font-semibold uppercase tracking-[0.14em] text-muted-foreground">
              Projects
            </p>
            <Badge variant="secondary" className="h-5 px-2 text-[10px]">
              {folderGroups.length}
            </Badge>
          </div>

          {loadingThreads ? (
            <p className="px-1.5 py-2 text-xs text-muted-foreground">
              Loading threads...
            </p>
          ) : folderGroups.length === 0 ? (
            <p className="px-1.5 py-2 text-xs text-muted-foreground">
              No sessions found in <code>~/.claude/projects</code>, <code>~/.codex/sessions</code>, or{" "}
              <code>~/.local/share/opencode/storage/session</code>.
            </p>
          ) : (
            <div className="min-h-0 flex-1 overflow-hidden">
              <style>{`
                .sidebar-scroll::-webkit-scrollbar {
                  width: 6px;
                }
                .sidebar-scroll::-webkit-scrollbar-track {
                  background: transparent;
                  border: none;
                }
                .sidebar-scroll::-webkit-scrollbar-thumb {
                  background: hsl(var(--muted-foreground) / 0.4);
                  border-radius: 3px;
                }
                .sidebar-scroll::-webkit-scrollbar-thumb:hover {
                  background: hsl(var(--muted-foreground) / 0.6);
                }
              `}</style>
              <div className="sidebar-scroll h-full overflow-y-auto pr-2.5">
                <ul className="w-full space-y-2 pb-1.5">
                  {folderGroups.map((group) => (
                    <ThreadFolderGroup
                      key={group.key}
                      group={group}
                      isActiveFolder={group.key === selectedFolderKey}
                      selectedThreadKey={selectedThreadKey}
                      onSelectThread={onSelectThread}
                      onCreateThread={onCreateThread}
                      isCreatingThread={creatingThreadFolderKey === group.key}
                      formatLastActive={formatLastActive}
                      getPreview={threadPreview}
                    />
                  ))}
                </ul>
              </div>
            </div>
          )}
        </div>

        <div className="border-t border-border/70 pt-2">
          <div ref={settingsRef} className="relative">
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="h-8 w-full items-center justify-between px-2.5 text-xs"
              onClick={() => setSettingsOpen((openState) => !openState)}
            >
              <span className="inline-flex items-center gap-1.5">
                <Settings2 className="h-3.5 w-3.5" />
                Settings
              </span>
              <ChevronUp
                className={cn(
                  "h-3.5 w-3.5 transition-transform",
                  settingsOpen ? "" : "rotate-180",
                )}
              />
            </Button>

            {settingsOpen ? (
              <div className="absolute bottom-full left-0 right-0 z-40 mb-2 rounded-md border border-border bg-card p-1.5 opacity-100 shadow-md">
                <p className="px-1.5 pb-1 text-[10px] uppercase tracking-[0.14em] text-muted-foreground">
                  Settings
                </p>
                <p className="px-1.5 pb-1 text-[10px] text-muted-foreground">
                  Active: {activeProviderOption?.label ?? "Claude Code"} / {activeProfileName}
                </p>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-8 w-full items-center justify-between px-2.5 text-xs"
                  onClick={openAccountDialog}
                >
                  <span className="inline-flex items-center gap-1.5">
                    {activeProviderOption ? (
                      <ProviderIcon
                        providerId={activeProviderOption.value}
                        className={cn("h-3.5 w-3.5", activeProviderOption.accentClass)}
                      />
                    ) : (
                      <ProviderIcon
                        providerId="claude_code"
                        className="h-3.5 w-3.5 text-[#FF7043] dark:text-[#FF8A65]"
                      />
                    )}
                    Agent Account
                  </span>
                  <span className="inline-flex items-center gap-1.5 text-muted-foreground">
                    {activeProfileName}
                    <ChevronRight className="h-3.5 w-3.5" />
                  </span>
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-8 w-full items-center justify-between px-2.5 text-xs"
                  onClick={openThemeDialog}
                >
                  <span className="inline-flex items-center gap-1.5">
                    {selectedThemeOption ? (
                      <selectedThemeOption.Icon className="h-3.5 w-3.5" />
                    ) : null}
                    App Theme
                  </span>
                  <span className="inline-flex items-center gap-1.5 text-muted-foreground">
                    {selectedThemeOption?.label ?? "Light"}
                    <ChevronRight className="h-3.5 w-3.5" />
                  </span>
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-8 w-full items-center justify-between px-2.5 text-xs"
                  onClick={() => {
                    setSkillsDialogOpen(true);
                    setSettingsOpen(false);
                  }}
                >
                  <span className="inline-flex items-center gap-1.5">
                    <Package className="h-3.5 w-3.5" />
                    Skills
                  </span>
                  <ChevronRight className="h-3.5 w-3.5" />
                </Button>
              </div>
            ) : null}
          </div>
        </div>
      </CardContent>

      {settingsOpen ? (
        <div
          className="fixed inset-0 z-30 bg-black/25"
          onClick={() => setSettingsOpen(false)}
        />
      ) : null}

      {newThreadDialogOpen ? (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/35 p-4"
          onClick={closeNewThreadDialog}
        >
          <Card
            className="w-full max-w-md border border-border bg-card opacity-100 shadow-xl"
            onClick={(event) => event.stopPropagation()}
          >
            <CardHeader className="pb-2">
              <CardTitle className="text-base">Create New Thread</CardTitle>
              <CardDescription className="text-xs">
                Choose project folder first, then provider.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="space-y-2">
                <p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                  1. Choose Project
                </p>
                <div className="max-h-40 space-y-1 overflow-y-auto rounded-md border border-border/70 p-1.5">
                  {folderGroups.length === 0 ? (
                    <p className="px-1.5 py-1 text-xs text-muted-foreground">
                      No existing projects in current thread list.
                    </p>
                  ) : (
                    folderGroups.map((group) => {
                      const active = group.key === selectedPathValue;
                      return (
                        <Button
                          key={group.key}
                          type="button"
                          variant={active ? "secondary" : "ghost"}
                          size="sm"
                          className="h-8 w-full items-center justify-between px-2 text-xs"
                          onClick={() => {
                            setSelectedProjectPath(group.key);
                            setPickerError(null);
                            setCreateDialogError(null);
                            setProviderInstallGuideError(null);
                            onClearError();
                            void loadProviderInstallStatuses(group.key);
                          }}
                          disabled={isCreateBusy}
                        >
                          <span className="inline-flex min-w-0 items-center gap-1.5">
                            <Folder className="h-3.5 w-3.5 shrink-0" />
                            <span className="truncate">{group.folderName}</span>
                          </span>
                          <span className="text-[10px] text-muted-foreground">
                            {group.threads.length}
                          </span>
                        </Button>
                      );
                    })
                  )}
                </div>

                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="h-8 w-full justify-start px-2 text-xs"
                  onClick={() => void handlePickFolder()}
                  disabled={isCreateBusy}
                >
                  {isPickingFolder ? (
                    <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <FolderSearch className="mr-1.5 h-3.5 w-3.5" />
                  )}
                  Choose local project
                </Button>

                {selectedPathValue ? (
                  <p className="text-[11px] text-muted-foreground">
                    Selected: <code>{selectedPathValue}</code>
                    {!hasSelectedFolderInList ? " (new)" : ""}
                  </p>
                ) : null}

                {pickerError ? (
                  <p className="text-[11px] text-destructive">{pickerError}</p>
                ) : null}
              </div>

              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                    2. Choose Provider
                  </p>
                  <Button
                    type="button"
                    variant="ghost"
                    size="sm"
                    className="h-6 px-1.5 text-[10px]"
                    onClick={() => void loadProviderInstallStatuses(selectedPathValue)}
                    disabled={isCreateBusy || providerStatusLoading}
                  >
                    {providerStatusLoading ? (
                      <Loader2 className="mr-1 h-3 w-3 animate-spin" />
                    ) : (
                      <RefreshCw className="mr-1 h-3 w-3" />
                    )}
                    Check
                  </Button>
                </div>
                <div className="grid grid-cols-1 gap-1.5">
                  {THREAD_PROVIDER_OPTIONS.map((provider) => {
                    const active = provider.value === selectedProviderId;
                    const status = providerInstallStatuses[provider.value];
                    return (
                      <Button
                        key={provider.value}
                        type="button"
                        variant={active ? "secondary" : "ghost"}
                        size="default"
                        className="h-auto w-full items-center justify-between px-2 py-1.5 text-xs"
                        onClick={() => {
                          setSelectedProviderId(provider.value);
                          setProviderInstallGuideError(null);
                        }}
                        disabled={isCreateBusy}
                      >
                        <span className="inline-flex min-w-0 items-center gap-1.5">
                          <ProviderIcon
                            providerId={provider.value}
                            className={cn("h-3.5 w-3.5", provider.accentClass)}
                          />
                          <span className="truncate">{provider.label}</span>
                        </span>
                        <span className="inline-flex items-center gap-1.5">
                          <span
                            className={cn(
                              "text-[10px] font-medium",
                              providerStatusClass(status, providerStatusLoading),
                            )}
                          >
                            {providerStatusLabel(status, providerStatusLoading)}
                          </span>
                          {active ? <Check className="h-3.5 w-3.5" /> : null}
                        </span>
                      </Button>
                    );
                  })}
                </div>

                {providerStatusError ? (
                  <p className="text-[11px] text-destructive">
                    Failed to check provider status: {providerStatusError}
                  </p>
                ) : null}

                {selectedProviderInstallStatus && !selectedProviderInstallStatus.installed ? (
                  <div className="space-y-2 rounded border border-destructive/30 bg-destructive/10 px-2 py-2">
                    <p className="inline-flex items-center gap-1.5 text-[11px] text-destructive">
                      <AlertTriangle className="h-3.5 w-3.5" />
                      {providerDisplayName(selectedProviderId)} CLI is not installed.
                    </p>
                    {selectedProviderInstallStatus.message ? (
                      <p className="text-[11px] text-destructive/90">
                        {selectedProviderInstallStatus.message}
                      </p>
                    ) : null}
                    <Button
                      type="button"
                      variant="outline"
                      size="sm"
                      className="h-7 px-2 text-[11px]"
                      onClick={() => void handleOpenInstallGuide()}
                      disabled={isCreateBusy}
                    >
                      Install {providerDisplayName(selectedProviderId)}
                      <ExternalLink className="ml-1.5 h-3 w-3" />
                    </Button>
                  </div>
                ) : null}

                {selectedProviderInstallStatus?.installed &&
                selectedProviderInstallStatus.healthStatus === "degraded" ? (
                  <p className="text-[11px] text-amber-700 dark:text-amber-400">
                    {selectedProviderInstallStatus.message ?? "Provider is installed but setup is incomplete."}
                  </p>
                ) : null}

                {providerInstallGuideError ? (
                  <p className="text-[11px] text-destructive">
                    Failed to open install guide: {providerInstallGuideError}
                  </p>
                ) : null}

                <p className="text-[11px] text-muted-foreground">
                  Launch profile: <code>{providerProfiles[selectedProviderId] ?? "default"}</code>
                </p>
              </div>

              {createStatusText ? (
                <p className="text-[11px] text-muted-foreground">{createStatusText}</p>
              ) : null}

              {visibleCreateError ? (
                <p className="rounded border border-destructive/30 bg-destructive/10 px-2 py-1.5 text-[11px] text-destructive">
                  {visibleCreateError}
                </p>
              ) : null}

              <div className="flex items-center justify-end gap-2 pt-1">
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-8 px-3 text-xs"
                  onClick={closeNewThreadDialog}
                  disabled={isCreateBusy}
                >
                  Cancel
                </Button>
                <Button
                  type="button"
                  variant="default"
                  size="sm"
                  className="h-8 px-3 text-xs"
                  onClick={() => void handleCreateFromDialog()}
                  disabled={!canCreate}
                >
                  {isCreateBusy ? (
                    <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                  ) : null}
                  {!selectedProviderStatusResolved
                    ? "Checking..."
                    : selectedProviderInstalled
                      ? "Create"
                      : "Install Required"}
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      ) : null}

      {accountDialogOpen ? (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/35 p-4"
          onClick={() => setAccountDialogOpen(false)}
        >
          <Card
            className="flex max-h-[86vh] w-full max-w-4xl flex-col border border-border bg-card opacity-100 shadow-xl"
            onClick={(event) => event.stopPropagation()}
          >
            <CardHeader className="pb-2">
              <CardTitle className="text-base">Agent Account</CardTitle>
              <CardDescription className="text-xs">
                Configure official and third-party suppliers per agent, then choose which one is active.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4 overflow-y-auto">
              <div className="space-y-1.5">
                <p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                  Agent
                </p>
                <div className="grid grid-cols-3 gap-1.5 sm:grid-cols-3">
                  {THREAD_PROVIDER_OPTIONS.map((provider) => {
                    const active = provider.value === pendingActiveProviderId;
                    return (
                      <Button
                        key={provider.value}
                        type="button"
                        variant={active ? "secondary" : "ghost"}
                        size="sm"
                        className="h-9 w-full items-center justify-center px-2 text-xs"
                        onClick={() => {
                          setPendingActiveProviderId(provider.value);
                          const nextActiveSupplierId =
                            pendingRuntimeSettings.activeSupplierIds[provider.value]
                            ?? OFFICIAL_SUPPLIER_ID;
                          setEditingSupplierId(nextActiveSupplierId);
                          setSelectedPresetId(firstPresetId(provider.value));
                          setConfigEditorNotice(null);
                        }}
                      >
                        <span className="inline-flex items-center gap-1.5">
                          <ProviderIcon
                            providerId={provider.value}
                            className={cn("h-3.5 w-3.5", provider.accentClass)}
                          />
                          <span className="truncate">{provider.label}</span>
                        </span>
                      </Button>
                    );
                  })}
                </div>
              </div>

              <div className="space-y-1.5">
                <p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                  Suppliers
                </p>
                <div className="flex flex-wrap items-center gap-1.5 rounded-md border border-border/70 bg-muted/20 p-2">
                  <Button
                    type="button"
                    variant="outline"
                    size="sm"
                    className="h-8 px-2 text-xs"
                    onClick={() => void importCcSwitchSuppliers()}
                    disabled={ccSwitchImporting}
                  >
                    {ccSwitchImporting ? (
                      <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                    ) : (
                      <RefreshCw className="mr-1.5 h-3.5 w-3.5" />
                    )}
                    Import from CC Switch
                  </Button>
                  <p className="text-[11px] text-muted-foreground">
                    Reads local <code>~/.cc-switch/cc-switch.db</code> and maps to current agent suppliers.
                  </p>
                </div>
                {ccSwitchImportNotice ? (
                  <p className="rounded border border-emerald-500/30 bg-emerald-500/10 px-2 py-1.5 text-[11px] text-emerald-700 dark:text-emerald-300">
                    {ccSwitchImportNotice}
                  </p>
                ) : null}
                {pendingProviderPresets.length > 0 ? (
                  <div className="space-y-2 rounded-md border border-border/70 bg-muted/20 p-2">
                    <p className="text-[11px] text-muted-foreground">
                      Add from preset
                    </p>
                    <div className="flex flex-wrap gap-1.5">
                      {pendingProviderPresets.map((preset) => {
                        const selected = preset.id === selectedPreset?.id;
                        return (
                          <Button
                            key={preset.id}
                            type="button"
                            variant={selected ? "secondary" : "ghost"}
                            size="sm"
                            className="h-7 rounded-full px-2.5 text-[11px]"
                            onClick={() => setSelectedPresetId(preset.id)}
                          >
                            {preset.label}
                          </Button>
                        );
                      })}
                    </div>
                    <div className="flex items-center gap-1.5">
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        className="h-8 px-2 text-xs"
                        onClick={addPendingPresetSupplier}
                        disabled={!selectedPreset}
                      >
                        <Plus className="mr-1.5 h-3.5 w-3.5" />
                        Add Preset
                      </Button>
                      {selectedPreset?.docsUrl ? (
                        <Button
                          type="button"
                          variant="ghost"
                          size="sm"
                          className="h-8 px-2 text-xs"
                          onClick={() => void openPresetDocs()}
                        >
                          Docs
                          <ExternalLink className="ml-1.5 h-3.5 w-3.5" />
                        </Button>
                      ) : null}
                    </div>
                    {selectedPreset ? (
                      <p className="text-[11px] text-muted-foreground">
                        {selectedPreset.description}
                      </p>
                    ) : null}
                  </div>
                ) : null}
                <div className="max-h-48 space-y-1.5 overflow-y-auto rounded-md border border-border/70 p-1.5">
                  {pendingProviderSuppliers.map((supplier) => {
                    const active = supplier.id === pendingActiveSupplierId;
                    const editing = supplier.id === editingSupplier?.id;
                    return (
                      <div
                        key={supplier.id}
                        className={cn(
                          "rounded-md border border-border/70 px-2 py-2",
                          active ? "border-primary/60 bg-primary/5" : "",
                        )}
                      >
                        <div className="flex items-center justify-between gap-2">
                          <div className="min-w-0">
                            <p className="truncate text-xs font-medium">
                              {supplier.name}
                            </p>
                            <p className="truncate text-[11px] text-muted-foreground">
                              {supplier.kind === "official" ? "Official Default" : "Third-party"} · profile{" "}
                              <code>{supplier.profileName}</code>
                            </p>
                            {supplier.note?.trim() ? (
                              <p className="truncate text-[11px] text-muted-foreground/90">
                                {supplier.note}
                              </p>
                            ) : null}
                          </div>
                          <div className="flex items-center gap-1">
                            <Button
                              type="button"
                              variant={active ? "secondary" : "outline"}
                              size="sm"
                              className="h-7 px-2 text-[11px]"
                              onClick={() => setPendingActiveSupplier(pendingActiveProviderId, supplier.id)}
                            >
                              {active ? "Enabled" : "Enable"}
                            </Button>
                            <Button
                              type="button"
                              variant={editing ? "secondary" : "ghost"}
                              size="sm"
                              className="h-7 px-2 text-[11px]"
                              onClick={() => {
                                setEditingSupplierId(supplier.id);
                                setConfigEditorNotice(null);
                              }}
                            >
                              Edit
                            </Button>
                            {supplier.kind === "custom" ? (
                              <Button
                                type="button"
                                variant="ghost"
                                size="sm"
                                className="h-7 px-2 text-[11px] text-destructive"
                                onClick={() => deletePendingSupplier(pendingActiveProviderId, supplier.id)}
                              >
                                Delete
                              </Button>
                            ) : null}
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>

              {editingSupplier ? (
                <div className="space-y-2 rounded-md border border-border/70 p-2">
                  <p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                    Edit Supplier
                  </p>
                  {editingOfficialSupplier ? (
                    <p className="rounded border border-border/70 bg-muted/30 px-2 py-1.5 text-[11px] text-muted-foreground">
                      Official supplier is read-only.
                    </p>
                  ) : null}

                  <div className="space-y-2 rounded-md border border-border/70 p-2">
                    <p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                      Basic
                    </p>
                    <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                      <label className="space-y-1">
                        <span className="text-[11px] text-muted-foreground">Name</span>
                        <input
                          type="text"
                          value={editingSupplier.name}
                          disabled={editingOfficialSupplier}
                          onChange={(event) =>
                            updatePendingSupplier(
                              pendingActiveProviderId,
                              editingSupplier.id,
                              { name: event.target.value },
                            )
                          }
                          className="h-8 w-full rounded-md border border-input bg-background px-2 text-xs outline-none focus:border-primary disabled:opacity-60"
                        />
                      </label>
                      <label className="space-y-1">
                        <span className="text-[11px] text-muted-foreground">Note</span>
                        <input
                          type="text"
                          value={editingSupplier.note ?? ""}
                          disabled={editingOfficialSupplier}
                          onChange={(event) =>
                            updatePendingSupplier(
                              pendingActiveProviderId,
                              editingSupplier.id,
                              { note: event.target.value },
                            )
                          }
                          placeholder="Optional note, e.g. team account"
                          className="h-8 w-full rounded-md border border-input bg-background px-2 text-xs outline-none focus:border-primary disabled:opacity-60"
                        />
                      </label>
                    </div>
                    <label className="space-y-1">
                      <span className="text-[11px] text-muted-foreground">Profile</span>
                      <input
                        type="text"
                        value={editingSupplier.profileName}
                        disabled={editingOfficialSupplier}
                        onChange={(event) =>
                          updatePendingSupplier(
                            pendingActiveProviderId,
                            editingSupplier.id,
                            { profileName: event.target.value },
                          )
                        }
                        className="h-8 w-full rounded-md border border-input bg-background px-2 text-xs outline-none focus:border-primary disabled:opacity-60"
                      />
                    </label>
                  </div>

                  <div className="space-y-2 rounded-md border border-border/70 p-2">
                    <p className="text-[11px] font-semibold uppercase tracking-[0.12em] text-muted-foreground">
                      Connection
                    </p>
                    <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                      <label className="space-y-1">
                        <span className="text-[11px] text-muted-foreground">API Base URL</span>
                        <input
                          type="text"
                          value={editingSupplier.baseUrl ?? ""}
                          disabled={editingOfficialSupplier}
                          onChange={(event) =>
                            updatePendingSupplier(
                              pendingActiveProviderId,
                              editingSupplier.id,
                              { baseUrl: event.target.value },
                            )
                          }
                          placeholder="https://..."
                          className="h-8 w-full rounded-md border border-input bg-background px-2 text-xs outline-none focus:border-primary disabled:opacity-60"
                        />
                      </label>
                      <label className="space-y-1">
                        <span className="text-[11px] text-muted-foreground">API Key</span>
                        <input
                          type="password"
                          value={editingSupplier.apiKey ?? ""}
                          disabled={editingOfficialSupplier}
                          onChange={(event) =>
                            updatePendingSupplier(
                              pendingActiveProviderId,
                              editingSupplier.id,
                              { apiKey: event.target.value },
                            )
                          }
                          placeholder="sk-..."
                          className="h-8 w-full rounded-md border border-input bg-background px-2 text-xs outline-none focus:border-primary disabled:opacity-60"
                        />
                      </label>
                    </div>
                  </div>

                  <div className="space-y-1.5 rounded-md border border-border/70 p-2">
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-[11px] text-muted-foreground">
                        {configProtocolHint(pendingActiveProviderId)}
                      </span>
                      <span
                        className={cn(
                          "text-[11px]",
                          editingConfigValidation.isValid
                            ? "text-emerald-600 dark:text-emerald-400"
                            : "text-destructive",
                        )}
                      >
                        {editingConfigValidation.isValid ? "Valid JSON" : "Invalid JSON"}
                      </span>
                    </div>
                    <div className="flex flex-wrap items-center gap-1.5">
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        className="h-7 px-2 text-[11px]"
                        onClick={applyConfigTemplate}
                        disabled={editingOfficialSupplier}
                      >
                        Template
                      </Button>
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        className="h-7 px-2 text-[11px]"
                        onClick={formatEditingConfigJson}
                        disabled={editingOfficialSupplier}
                      >
                        Format
                      </Button>
                      <Button
                        type="button"
                        variant="outline"
                        size="sm"
                        className="h-7 px-2 text-[11px]"
                        onClick={validateEditingConfigJson}
                        disabled={editingOfficialSupplier}
                      >
                        Validate
                      </Button>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-7 px-2 text-[11px]"
                        onClick={clearEditingConfigJson}
                        disabled={editingOfficialSupplier}
                      >
                        Clear
                      </Button>
                    </div>
                    <JsonCodeEditor
                      value={editingSupplier.configJson ?? ""}
                      disabled={editingOfficialSupplier}
                      darkMode={configEditorDarkMode}
                      invalid={!editingConfigValidation.isValid}
                      minHeight={240}
                      onChange={(nextValue) => {
                        setConfigEditorNotice(null);
                        updatePendingSupplier(
                          pendingActiveProviderId,
                          editingSupplier.id,
                          { configJson: nextValue },
                        );
                      }}
                    />
                    <p
                      className={cn(
                        "text-[11px]",
                        editingConfigValidation.isValid
                          ? "text-muted-foreground"
                          : "text-destructive",
                      )}
                    >
                      {editingConfigValidation.message}
                    </p>
                    {configEditorNotice ? (
                      <p
                        className={cn(
                          "text-[11px]",
                          configEditorNotice.tone === "success"
                            ? "text-emerald-600 dark:text-emerald-400"
                            : "text-destructive",
                        )}
                      >
                        {configEditorNotice.message}
                      </p>
                    ) : null}
                  </div>
                </div>
              ) : null}

              <p className="text-[11px] text-muted-foreground">
                Current target: {pendingActiveProviderOption?.label ?? "Claude Code"} /{" "}
                {pendingActiveProfileName.trim() || "default"}
              </p>

              <div className="rounded border border-border/60 bg-muted/30 px-2 py-1.5 text-[11px] text-muted-foreground">
                Launch profile preview for {providerDisplayName(pendingActiveProviderId)}:{" "}
                <code>{resolveProviderActiveProfile(pendingRuntimeSettings, pendingActiveProviderId)}</code>
              </div>

              {accountDialogError ? (
                <p className="rounded border border-destructive/30 bg-destructive/10 px-2 py-1.5 text-[11px] text-destructive">
                  {accountDialogError}
                </p>
              ) : null}

              <div className="flex items-center justify-end gap-2 pt-1">
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-8 px-3 text-xs"
                  onClick={() => setAccountDialogOpen(false)}
                >
                  Cancel
                </Button>
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  className="h-8 px-3 text-xs"
                  onClick={applyActiveAgentProfileChange}
                >
                  Apply
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      ) : null}

      {themeDialogOpen ? (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/35 p-4"
          onClick={() => setThemeDialogOpen(false)}
        >
          <Card
            className="w-full max-w-sm border border-border bg-card opacity-100 shadow-xl"
            onClick={(event) => event.stopPropagation()}
          >
            <CardHeader className="pb-2">
              <CardTitle className="text-base">App Theme</CardTitle>
              <CardDescription className="text-xs">
                Choose the appearance for the entire desktop app.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-2">
              {APP_THEME_OPTIONS.map(({ value, label, Icon }) => (
                <Button
                  key={value}
                  type="button"
                  variant={pendingTheme === value ? "secondary" : "ghost"}
                  size="sm"
                  className="h-9 w-full items-center justify-between px-2.5 text-xs"
                  onClick={() => setPendingTheme(value)}
                >
                  <span className="inline-flex items-center gap-1.5">
                    <Icon className="h-3.5 w-3.5" />
                    {label}
                  </span>
                  {pendingTheme === value ? <Check className="h-3.5 w-3.5" /> : null}
                </Button>
              ))}
              <div className="flex items-center justify-end gap-2 pt-1">
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="h-8 px-3 text-xs"
                  onClick={() => setThemeDialogOpen(false)}
                >
                  Cancel
                </Button>
                <Button
                  type="button"
                  variant="secondary"
                  size="sm"
                  className="h-8 px-3 text-xs"
                  onClick={applyThemeChange}
                >
                  Apply
                </Button>
              </div>
              <p className="text-[11px] text-muted-foreground">
                Current: {pendingThemeOption?.label ?? "Light"}
              </p>
            </CardContent>
          </Card>
        </div>
      ) : null}

      {skillsDialogOpen ? (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/35 p-4"
          onClick={() => setSkillsDialogOpen(false)}
        >
          <Card
            className="w-full max-w-md border border-border bg-card opacity-100 shadow-xl"
            onClick={(event) => event.stopPropagation()}
          >
            <CardHeader className="pb-2">
              <CardTitle className="text-base">Skills</CardTitle>
              <CardDescription className="text-xs">
                Manage installed skills for agent providers.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <SkillsPanel appTheme={appTheme} />
            </CardContent>
          </Card>
        </div>
      ) : null}
    </Card>
  );
}
