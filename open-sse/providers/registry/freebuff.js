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
    // Freebuff's free tier is gated to its own CLI runtime, so direct API
    // calls get 403 free_mode_invalid_agent_model. The community router
    // freebuff2api (npm) runs the real Freebuff protocol locally and exposes
    // an OpenAI-compatible surface — point this provider at it:
    //   npm i -g freebuff2api && set FREEBUFF_TOKEN=<freebuff login token>
    //   freebuff2api   (serves http://127.0.0.1:8787/v1)
    baseUrl: "http://127.0.0.1:8787/v1/chat/completions",
    validateUrl: "http://127.0.0.1:8787/v1/models",
    thinkingFormat: "openai",
    minMaxTokens: 4096,
  },
  models: [
    { id: "mimo/mimo-v2.5", name: "MiMo 2.5 (Freebuff)" },
    { id: "deepseek/deepseek-v4-flash", name: "DeepSeek V4 Flash (Freebuff)" },
    { id: "minimax/minimax-m3", name: "MiniMax M3 (Freebuff)" },
    { id: "openai/gpt-5.6-luna", name: "GPT-5.6 Luna (Freebuff)" },
    { id: "deepseek/deepseek-v4-pro", name: "DeepSeek V4 Pro (Freebuff)" },
  ],
};
