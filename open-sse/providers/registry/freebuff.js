// Freebuff / Codebuff (freebuff.com / codebuff.com)
// Built on the Codebuff platform. Offers free & unmetered coding models:
// DeepSeek V4 Flash, GLM 5.3 Flash, GPT-5.6 Luna, MiMo 2.5, Solar Pro 4.
export default {
  id: "freebuff",
  priority: 106,
  alias: "freebuff",
  aliases: ["fb", "codebuff", "cb"],
  uiAlias: "fb",
  display: {
    name: "Freebuff",
    icon: "smart_toy",
    color: "#3B82F6",
    textIcon: "FB",
    website: "https://freebuff.com",
    notice: {
      apiKeyUrl: "https://freebuff.com",
      text: "Free AI coding agent platform. Use with Freebuff/Codebuff session token or API key.",
    },
  },
  category: "apikey",
  authType: "apikey",
  authModes: ["apikey"],
  serviceKinds: ["llm"],
  transport: {
    // Freebuff is a Codebuff rebrand: the CLI binary posts chat requests to
    // codebuff.com/api/v1/chat/completions (verified in freebuff.exe — the
    // url builder is path.join("/api/v1", "/chat/completions") on FP() base).
    // Auth = Bearer token from `freebuff login` (stored in
    // ~/.config/manicode/auth.json / CODEBUFF_API_KEY env).
    baseUrl: "https://www.codebuff.com/api/v1/chat/completions",
    validateUrl: "https://www.codebuff.com/api/v1/me",
    // Their backend mirrors the exact UA the CLI sends (ai-sdk/openai-compatible/x/codebuff).
    headers: {
      "User-Agent": "ai-sdk/openai-compatible/1.0.0/codebuff",
    },
    thinkingFormat: "openai",
    minMaxTokens: 4096,
  },
  models: [
    { id: "deepseek-v4-flash", name: "DeepSeek V4 Flash (07/31)", default: true },
    { id: "glm-5.3-flash", name: "GLM 5.3 Flash", maxOutputTokens: 8192 },
    { id: "gpt-5.6-luna", name: "GPT-5.6 Luna" },
    { id: "mimo-v2.5", name: "MiMo 2.5" },
    { id: "solar-pro-4", name: "Solar Pro 4" },
  ],
};
