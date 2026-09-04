// SeekAI — OpenAI-compatible unified LLM gateway (seekai.cc).
// Single API key, POST /v1/chat/completions, Bearer auth. Streaming, tool/function calling and
// JSON mode all work over the standard OpenAI Chat Completions wire format — no custom executor,
// no translation hop needed.
//
// Replaces AgentRouter (agentrouter.org) as the user's preferred gateway.
export default {
  id: "seekai",
  priority: 105,
  alias: "seekai",
  aliases: ["sa"],
  uiAlias: "sa",
  display: {
    name: "SeekAI",
    icon: "bolt",
    color: "#10B981",
    textIcon: "SA",
    website: "https://seekai.cc",
    notice: {
      apiKeyUrl: "https://seekai.cc/token",
      text: "OpenAI-compatible gateway. Get a key from the console.",
    },
  },
  category: "apikey",
  authType: "apikey",
  authModes: ["apikey"],
  transport: {
    baseUrl: "https://seekai.cc/v1/chat/completions",
    validateUrl: "https://seekai.cc/v1/models",
    thinkingFormat: "openai",
  },
  models: [
    { id: "minimax-m3", name: "MiniMax M3" },
    { id: "grok-4.6", name: "Grok 4.6" },
    { id: "kimi-k3", name: "Kimi K3" },
    { id: "mimo-v2.5", name: "Mimo v2.5" },
    { id: "glm-5.3-flash", name: "GLM-5.3 Flash" },
    { id: "deepseek-v4-flash-vision-exp", name: "DeepSeek V4 Flash Vision" },
    { id: "deepseek-v4-flash", name: "DeepSeek V4 Flash" },
  ],
};
