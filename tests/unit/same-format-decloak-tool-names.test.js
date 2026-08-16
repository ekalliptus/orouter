import { describe, it, expect } from "bun:test";
import { translateResponse } from "../../open-sse/translator/index.js";
import { FORMATS } from "../../open-sse/translator/formats.js";

// Request-side Claude OAuth cloaking renames client tools (Skill → Skill_ide).
// Same-format response streams used to pass chunks through untouched, so the
// client received the suffixed name and failed with "Tool not found: Skill_ide".
describe("same-format translateResponse tool-name decloak", () => {
  const state = { toolNameMap: new Map([["Skill_ide", "Skill"], ["Bash_ide", "Bash"]]) };

  it("restores the original name in streaming content_block_start events", () => {
    const chunk = {
      type: "content_block_start",
      index: 0,
      content_block: { type: "tool_use", id: "toolu_01ABC", name: "Skill_ide", input: {} },
    };
    const [out] = translateResponse(FORMATS.CLAUDE, FORMATS.CLAUDE, chunk, state);
    expect(out.content_block.name).toBe("Skill");
  });

  it("restores names in full-message content arrays", () => {
    const chunk = {
      content: [
        { type: "text", text: "hi" },
        { type: "tool_use", id: "toolu_02", name: "Bash_ide", input: { cmd: "ls" } },
      ],
    };
    const [out] = translateResponse(FORMATS.CLAUDE, FORMATS.CLAUDE, chunk, state);
    expect(out.content[1].name).toBe("Bash");
    expect(out.content[0]).toBe(chunk.content[0]);
  });

  it("passes non-tool chunks through unchanged", () => {
    const chunk = { type: "message_delta", delta: { stop_reason: "tool_use" } };
    const [out] = translateResponse(FORMATS.CLAUDE, FORMATS.CLAUDE, chunk, state);
    expect(out).toBe(chunk);
  });

  it("is a no-op without a toolNameMap (plain passthrough)", () => {
    const chunk = {
      type: "content_block_start",
      index: 0,
      content_block: { type: "tool_use", id: "toolu_03", name: "Skill", input: {} },
    };
    const [out] = translateResponse(FORMATS.CLAUDE, FORMATS.CLAUDE, chunk, {});
    expect(out.content_block.name).toBe("Skill");
  });

  it("leaves unmapped (decoy) tool names alone", () => {
    const chunk = {
      type: "content_block_start",
      index: 0,
      content_block: { type: "tool_use", id: "toolu_04", name: "WebSearch", input: {} },
    };
    const [out] = translateResponse(FORMATS.CLAUDE, FORMATS.CLAUDE, chunk, state);
    expect(out.content_block.name).toBe("WebSearch");
  });
});
