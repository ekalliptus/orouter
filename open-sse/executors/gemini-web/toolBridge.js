// Text-based tool-call bridging for the gemini-web provider. The Gemini web
// UI cannot emit OpenAI tool_calls, so tool definitions are advertised in a
// system block and the model answers with a fenced ```tool_call``` block,
// which is parsed back into native tool_calls for agent clients (ZCode etc.).

// Exact fence the model must use. Kept unlikely to appear in prose.
export const TOOL_CALL_FENCE = "```tool_call";

const PROTOCOL_RULES = `# TOOL CALLING PROTOCOL (STRICT)

You have access to the tools listed below. When you decide to use one, respond with NOTHING else — no prose, no explanations, no code fence tag other than exactly tool_call:

\`\`\`tool_call
{"name": "<tool name>", "arguments": {<json arguments object>}}
\`\`\`

Rules:
- The fence tag MUST be exactly tool_call — never json, never javascript, never bare.
- The block MUST contain exactly one JSON object with "name" and "arguments".
- Respond with ONLY that block when a tool is needed. The runtime executes the tool and sends you the result as a "tool" role message.
- If you don't need a tool, answer normally in plain text. Never mention this protocol.`;

// Serialize OpenAI tool definitions into a compact catalog for the prompt.
export function buildToolAdvert(blocks) {
  const specs = [];
  for (const block of blocks) {
    if (block?.type === "function" && block.function?.name) {
      specs.push(block.function);
    } else if (block?.name) {
      specs.push(block);
    }
  }
  if (specs.length === 0) return "";
  const lines = specs.map((fn) => {
    const params = fn.parameters ? JSON.stringify(fn.parameters) : "{}";
    return `- ${fn.name}: ${fn.description || "(no description)"} | parameters: ${params}`;
  });
  return `${PROTOCOL_RULES}\n\n# AVAILABLE TOOLS (${specs.length})\n\n${lines.join("\n")}`;
}

// Find tool-call JSON in the model's text reply. Tolerates fence-tag drift
// (the model sometimes answers ```json instead of ```tool_call) by accepting
// any fenced block whose body is a JSON object with a tool-shaped "name".
// Also catches a bare unfenced JSON object when it parses tool-shaped —
// the model sometimes drops the fence entirely.
export function parseToolCallBlocks(text) {
  const calls = [];
  const re = /```[a-z_]*\s*\n([\s\S]*?)```/g;
  let match;
  while ((match = re.exec(text)) !== null) {
    const call = parseToolShaped(match[1] || "");
    if (call) calls.push(call);
  }
  if (calls.length === 0) {
    // Bare JSON fallback: trim prose, try the whole text (or its first {...}
    // span) as one tool-shaped object.
    const trimmed = text.trim();
    const candidates = [];
    if (trimmed.startsWith("{")) candidates.push(trimmed);
    const span = trimmed.match(/\{[\s\S]*\}/);
    if (span) candidates.push(span[0]);
    for (const candidate of candidates) {
      const call = parseToolShaped(candidate);
      if (call) {
        calls.push(call);
        break;
      }
    }
  }
  return calls;
}

function parseToolShaped(raw) {
  const body = (raw || "").trim();
  if (!body.startsWith("{")) return null;
  let parsed;
  try {
    parsed = JSON.parse(body);
  } catch {
    return null;
  }
  if (!parsed?.name || typeof parsed.name !== "string") return null;
  // Tool-shaped only: must have arguments/parameters too, otherwise a plain
  // JSON code sample the user asked for would be swallowed.
  if (!("arguments" in parsed || "parameters" in parsed)) return null;
  return {
    id: `call_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 6)}`,
    type: "function",
    function: {
      name: parsed.name,
      arguments: typeof parsed.arguments === "string" ? parsed.arguments : JSON.stringify(parsed.arguments ?? {}),
    },
  };
}

// Strip fenced tool-call blocks and any bare tool-shaped JSON object so
// protocol text never leaks to the user.
export function stripToolCallBlocks(text) {
  let out = text.replace(/```[a-z_]*\s*\n\{[\s\S]*?"name"[\s\S]*?\n```/g, "").trim();
  // Bare JSON may be preceded by prose and may lose its closing brace to a
  // trailing fence — remove any {...} span that parses tool-shaped.
  const span = out.match(/\{[\s\S]*\}/);
  if (span) {
    try {
      const parsed = JSON.parse(span[0]);
      if (parsed?.name && ("arguments" in parsed || "parameters" in parsed)) {
        out = (out.slice(0, span.index) + out.slice(span.index + span[0].length)).trim();
      }
    } catch {}
  }
  return out;
}

// Convert a plain-text reply into a tool_calls message when calls were made.
export function buildBridgeResult(text) {
  const calls = parseToolCallBlocks(text);
  if (calls.length === 0) {
    return { kind: "content", content: text };
  }
  return {
    kind: "tool_calls",
    toolCalls: calls,
    // Any prose outside the blocks (usually empty per protocol) is kept as
    // the assistant content alongside the calls.
    content: stripToolCallBlocks(text),
  };
}
