// Gemini Web executor — drives the real gemini.google.com/app UI in a
// headless Chromium (playwright-core) with the user's cookies. Reconstructed
// from upstream 9router 0.5.109 webpack chunk 5330 module 29166; browserLease
// ships verbatim from the upstream package.

import { BaseExecutor } from "./base.js";
import { PROVIDERS } from "../config/providers.js";
import { SSE_DONE, SSE_HEADERS_NO_BUFFER } from "../utils/sseConstants.js";
import { normalizeCookieInput, toPlaywrightCookies } from "../utils/webCookieString.js";
import { findChromePath, launchChromium, isBrowserMissingError } from "../lib/browserBinary.js";
import { lazyImport } from "../lib/lazyImport.js";

const GEMINI_APP_URL = "https://gemini.google.com/app";
const GEMINI_WEB_UA =
  "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/149.0.0.0 Safari/537.36";

const UNSUPPORTED_CONTROL_CODE = "unsupported_control_for_provider";
const NO_THINKING_EFFORTS = new Set(["none", "minimal"]);

// UI mode descriptors — how to switch the Gemini web UI to a given model and
// how to confirm the switch took effect. gemini-3.1-pro is the app default.
const MODEL_DESCRIPTORS = {
  "gemini-3.1-pro": { id: "gemini-3.1-pro", isDefault: true },
  "gemini-4-argon": {
    id: "gemini-4-argon",
    isDefault: false,
    toggleSelector: '[data-test-id="bard-mode-menu-button"], button[aria-haspopup="menu"][aria-label*="model" i]',
    activeIndicatorSelector: '[data-test-id="bard-mode-menu-button"] .mode-title, [data-test-id="bard-mode-menu-button"]',
    // No \b after "argon": the UI glues the thinking level to the model name
    // ("ArgonTinggi") so a trailing word boundary never matches.
    expectedIndicatorPattern: /argon/i,
  },
  "gemini-3.7-flash": {
    id: "gemini-3.7-flash",
    isDefault: false,
    toggleSelector: '[data-test-id="bard-mode-menu-button"], button[aria-haspopup="menu"][aria-label*="model" i]',
    activeIndicatorSelector: '[data-test-id="bard-mode-menu-button"] .mode-title, [data-test-id="bard-mode-menu-button"]',
    expectedIndicatorPattern: /\bflash\b(?!.*\blite\b)/i,
  },
  "gemini-3.1-flash-lite": {
    id: "gemini-3.1-flash-lite",
    isDefault: false,
    toggleSelector: '[data-test-id="bard-mode-menu-button"], button[aria-haspopup="menu"][aria-label*="model" i]',
    activeIndicatorSelector: '[data-test-id="bard-mode-menu-button"] .mode-title, [data-test-id="bard-mode-menu-button"]',
    expectedIndicatorPattern: /flash.*\blite\b/i,
  },
};

const EXTENDED_THINKING_DESCRIPTOR = {
  id: "extended-thinking",
  isDefault: false,
  toggleSelector: '[data-test-id="deep-think-toggle"], button[aria-label*="extended thinking" i], button[aria-label*="deep think" i]',
  activeIndicatorSelector: '[data-test-id="deep-think-toggle"][aria-pressed="true"], [data-test-id="deep-think-toggle"].is-active',
  expectedIndicatorPattern: /thinking|deep think/i,
};

const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function readControlText(el) {
  try {
    if (typeof el.textContent === "function") {
      const text = await el.textContent();
      if (typeof text === "string") return text;
    }
    if (typeof el.innerText === "function") return await el.innerText();
  } catch {}
  return null;
}

// Click a UI toggle, attempt to select the matching menu item (for model
// descriptors), and confirm the active-mode indicator now matches.
async function confirmControl(page, descriptor, timeoutMs = 5000) {
  if (descriptor.isDefault) return { confirmed: true, descriptor };
  const { toggleSelector, activeIndicatorSelector, expectedIndicatorPattern } = descriptor;
  if (!toggleSelector || !activeIndicatorSelector || !expectedIndicatorPattern) {
    return { confirmed: false, reason: "control_not_found", descriptor };
  }
  // Retry the open-menu → select-item → verify cycle: the menu sometimes
  // renders after the first toggle click (fresh chats hydrate late).
  let lastSeen = "";
  for (let attempt = 0; attempt < 3; attempt++) {
    const toggle = await page.waitForSelector(toggleSelector, { timeout: timeoutMs }).catch(() => null);
    if (!toggle) return { confirmed: false, reason: "control_not_found", descriptor };
    await toggle.click();
    await page.waitForTimeout(900);
    // Model menus list selectable items; extended-thinking is a plain toggle.
    // Try to click the item matching the pattern before reading the indicator —
    // upstream only opened the menu, which verifies but never switches.
    // NOTE: Gemini's menu items are plain elements (role="" on current DOM), so
    // match menu descendants broadly and click via DOM el.click() — playwright
    // locator clicks with actionability checks miss these overlay items.
    if (descriptor.id !== "extended-thinking") {
      try {
        await page.evaluate((patternSource) => {
          const re = new RegExp(patternSource, "i");
          const all = [...document.querySelectorAll('[role="menuitem"], [role="option"], [role="menu"] *, [role="listbox"] *')];
          const el = all.find((e) => {
            const t = (e.textContent || "").trim();
            return t && t.length < 40 && re.test(t);
          });
          if (el) el.click();
        }, expectedIndicatorPattern.source);
        await page.waitForTimeout(1500);
      } catch {}
    }
    const indicator = await page.waitForSelector(activeIndicatorSelector, { timeout: timeoutMs }).catch(() => null);
    if (!indicator) return { confirmed: false, reason: "indicator_not_found", descriptor };
    const text = await readControlText(indicator);
    lastSeen = text || "";
    if (text && expectedIndicatorPattern.test(text)) return { confirmed: true, descriptor };
    // Menu may have closed without selecting — reopen on the next attempt.
    await page.waitForTimeout(1000);
  }
  return { confirmed: false, reason: "indicator_mismatch", descriptor, indicatorText: lastSeen };
}

async function confirmModelMode(page, model, timeoutMs) {
  const descriptor = MODEL_DESCRIPTORS[model] ?? null;
  if (!descriptor) return { confirmed: false, reason: "unknown_model" };
  return confirmControl(page, descriptor, timeoutMs);
}

const UNSWITCHABLE_REASONS = {
  unknown_model: "it is not one of the advertised, selectable gweb models",
  control_not_found: "the Gemini UI control used to switch modes was not found",
  indicator_not_found: "the active-mode indicator used to confirm the switch was not found",
  indicator_mismatch: "the active-mode indicator did not confirm the switch after attempting it",
};

function unsupportedControlMessage(model, reason) {
  const detail = UNSWITCHABLE_REASONS[reason] || "the switch could not be verified";
  return `Model provider "gemini-web" could not verify that "${model}" is actually active in the Gemini web UI (${detail}). Rather than silently answering under a model/mode label that may not be accurate, the request was rejected with "unsupported_control_for_provider".`;
}

// Snapshot of the last model-response in the Gemini UI DOM.
const READ_LAST_MESSAGE_SCRIPT = `(() => {
  const msgs = [...document.querySelectorAll("model-response, message-content")];
  const last = msgs.length ? (msgs[msgs.length - 1].innerText || '').trim() : '';
  return {
    count: msgs.length,
    lastText: last,
    generating: !!document.querySelector('[data-test-id="thinking-overlay-content"], button[aria-label*="Stop" i]'),
  };
})()`;

async function readChatState(page) {
  try {
    return (await page.evaluate(READ_LAST_MESSAGE_SCRIPT)) || { count: 0, lastText: "", generating: false };
  } catch {
    return { count: 0, lastText: "", generating: false };
  }
}

// Poll the DOM until the reply settles, emitting content deltas as they grow.
async function pollTextReply({ page, baseCount = 0, baseText = "", timeoutMs = 60000, firstTokenMs = 30000, signal, onDelta }) {
  const startedAt = Date.now();
  let emitted = "";
  let lastSeen = "";
  let stableCount = 0;
  let gotFirst = false;
  for (;;) {
    if (signal?.aborted) throw Error("Request aborted");
    const elapsed = Date.now() - startedAt;
    if (elapsed > timeoutMs) throw Error("Gemini reply timed out");
    if (!gotFirst && elapsed > firstTokenMs) throw Error("no first token from Gemini");
    await delay(400);
    const state = await readChatState(page);
    const text = state.lastText || "";
    if ((state.count > baseCount || (text && text !== baseText)) && text) {
      gotFirst = true;
      if (text !== lastSeen) {
        lastSeen = text;
        stableCount = 0;
      } else {
        stableCount += 1;
      }
      if (!state.generating && text !== emitted) {
        const delta = text.startsWith(emitted) ? text.slice(emitted.length) : text;
        if (delta) {
          emitted = text;
          onDelta?.(delta);
        }
      }
      if (stableCount >= 20) return emitted || text;
      if (!state.generating && stableCount >= 5 && text === emitted) return emitted;
    }
  }
}

// Snapshot of generated images in the last model-response.
const READ_IMAGES_SCRIPT = `(() => {
  const msgs = [...document.querySelectorAll('model-response, message-content')];
  const last = msgs.length ? msgs[msgs.length - 1] : null;
  const imgs = last
    ? [...last.querySelectorAll('img')].map((i) => ({
        src: i.currentSrc || i.src || '',
        w: i.naturalWidth || 0,
        h: i.naturalHeight || 0,
      }))
    : [];
  return {
    count: msgs.length,
    generating: !!document.querySelector('[data-test-id="thinking-overlay-content"], button[aria-label*="Stop" i]'),
    images: imgs.filter((i) => i.src && !i.src.includes('gstatic') && i.w > 0),
  };
})()`;

async function readImageState(page) {
  try {
    return (await page.evaluate(READ_IMAGES_SCRIPT)) || { count: 0, generating: false, images: [] };
  } catch {
    return { count: 0, generating: false, images: [] };
  }
}

// In-page download of a generated image → base64 PNG.
async function downloadGeneratedImage(page, src) {
  const result = await page.evaluate(async (imageSrc) => {
    if (imageSrc.startsWith("data:")) {
      const res = await fetch(imageSrc);
      return {
        ok: true,
        mime: (await res.blob()).type || "image/png",
        b64: imageSrc.slice(imageSrc.indexOf(",") + 1),
      };
    }
    try {
      const img = await new Promise((resolve, reject) => {
        const el = new Image();
        el.onload = () => resolve(el);
        el.onerror = () => reject(Error("img load failed"));
        el.src = imageSrc;
      });
      const canvas = document.createElement("canvas");
      canvas.width = img.naturalWidth;
      canvas.height = img.naturalHeight;
      canvas.getContext("2d").drawImage(img, 0, 0);
      const dataUrl = canvas.toDataURL("image/png");
      return { ok: true, mime: "image/png", b64: dataUrl.slice(dataUrl.indexOf(",") + 1) };
    } catch {}
    try {
      const res = await fetch(imageSrc);
      if (!res.ok) return { ok: false, err: `status ${res.status}` };
      const blob = await res.blob();
      const bytes = new Uint8Array(await blob.arrayBuffer());
      let binary = "";
      for (let i = 0; i < bytes.length; i += 32768) binary += String.fromCharCode(...bytes.subarray(i, i + 32768));
      return { ok: true, mime: blob.type || "image/png", b64: btoa(binary) };
    } catch (err) {
      return { ok: false, err: String(err) };
    }
  }, src);
  if (!result?.ok) throw Error(`failed to download generated image: ${result?.err}`);
  return { b64: result.b64, mime: result.mime };
}

// Poll the DOM until a generated image appears, then download it.
async function pollImageReply(page, { timeoutMs = 120000, signal }) {
  const startedAt = Date.now();
  let sawActivity = false;
  for (;;) {
    if (signal?.aborted) throw Error("Request aborted");
    if (Date.now() - startedAt > timeoutMs) throw Error("Gemini image generation timed out");
    await delay(1000);
    const state = await readImageState(page);
    if ((state.count > 0 || state.generating) && !sawActivity) sawActivity = true;
    if (!sawActivity && Date.now() - startedAt > 30000) throw Error("no first sign of activity from Gemini");
    const ready = state.images.filter((img) => img.w >= 100);
    if (ready.length > 0 && !state.generating) {
      const last = ready[ready.length - 1];
      return { ...(await downloadGeneratedImage(page, last.src)), width: last.w, height: last.h };
    }
  }
}

async function launchGeminiBrowser(options) {
  const { chromium } = await lazyImport("playwright-core");
  const executablePath = findChromePath();
  return launchChromium(chromium, {
    ...options,
    ...(executablePath ? { executablePath } : {}),
  });
}

// Google DBSC (Device-Bound Session Credentials) binds web sessions to the
// browser that created them — replaying __Secure-1PSID cookies into a fresh
// automated browser gets an anonymous session (verified 2026-10-25). The
// working path is attaching to the user's REAL logged-in Chrome over CDP.
// Set GEMINI_WEB_CDP_ENDPOINT (or OBSCURA_CDP_ENDPOINT) to e.g.
// http://127.0.0.1:9222 with Chrome started via --remote-debugging-port=9222.
function cdpEndpoint() {
  return process.env.GEMINI_WEB_CDP_ENDPOINT || process.env.OBSCURA_CDP_ENDPOINT || "";
}

async function acquireSession(cookieHeader) {
  const endpoint = cdpEndpoint();
  if (endpoint) {
    const { chromium } = await lazyImport("playwright-core");
    const browser = await chromium.connectOverCDP(endpoint);
    const context = browser.contexts()[0] || (await browser.newContext({ userAgent: GEMINI_WEB_UA }));
    const page = await context.newPage();
    return { browser, context, page, owned: false };
  }
  const { acquireGeminiBrowser, releaseGeminiBrowser } = await import("./gemini-web/browserLease.js");
  const browser = await acquireGeminiBrowser(launchGeminiBrowser);
  const context = await browser.newContext({ userAgent: GEMINI_WEB_UA });
  if (cookieHeader) {
    await context.addCookies(toPlaywrightCookies(cookieHeader, ".google.com"));
  }
  const page = await context.newPage();
  return { browser, context, page, owned: true, releaseGeminiBrowser };
}

function buildNonStreamResponse(content, model, finishReason = "stop") {
  return {
    id: `chatcmpl-${Date.now()}`,
    object: "chat.completion",
    created: Math.floor(Date.now() / 1000),
    model,
    choices: [
      {
        index: 0,
        message: { role: "assistant", content },
        finish_reason: finishReason,
        logprobs: null,
      },
    ],
    usage: { prompt_tokens: 0, completion_tokens: 0, total_tokens: 0 },
  };
}

function buildStreamChunk(deltaContent, model, finishReason = null) {
  return {
    id: `chatcmpl-${Date.now()}`,
    object: "chat.completion.chunk",
    created: Math.floor(Date.now() / 1000),
    model,
    choices: [{ index: 0, delta: deltaContent ? { content: deltaContent } : {}, finish_reason: finishReason }],
  };
}

function errorResponse(status, error) {
  return new Response(JSON.stringify({ error }), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

const REFUSAL_RE = /something went wrong|try again later|unusual traffic|verify you are human/i;
const QUOTA_RE = /used all of your requests|quota will reset|quota exceeded|reached your limit/i;

// Returns a friendly error string when the page is clearly not a usable chat.
async function detectPageProblem(page, getQuotaError) {
  if ((await page.locator(".ql-editor, [contenteditable='true']").count()) > 0) return null;
  const quotaError = getQuotaError();
  if (quotaError) return quotaError;
  const bodyText = await page
    .evaluate(() => document.body?.innerText?.slice(0, 2000) ?? "")
    .catch(() => "");
  if (REFUSAL_RE.test(bodyText)) {
    return 'Gemini refused the request ("Something went wrong") — the Google account cannot open Gemini. Sign in again, or enable Gemini Apps Activity for the account';
  }
  const url = page.url();
  if (/chat with gemini/i.test(bodyText) || !/\/app(\/|$|\?)/.test(url)) {
    for (let i = 0; i < 12; i++) {
      const err = getQuotaError();
      if (err) return err;
      await page.waitForTimeout(400);
    }
    return "Gemini did not open a chat session — open dashboard/providers/gemini-web, click Login via Browser and sign in to Google again (the account may also need Gemini Apps Activity enabled)";
  }
  return null;
}

// Navigate to /app and start listening for quota-rejection batchexecute replies.
async function openGeminiApp(page) {
  let quotaError = null;
  const stopListening = (() => {
    let seen = false;
    page.on("response", (res) => {
      if (seen) return;
      if (/\/BardChatUi\/data\/batchexecute/.test(res.url())) {
        res
          .text()
          .then((body) => {
            if (QUOTA_RE.test(body)) {
              seen = true;
              quotaError =
                'Gemini says this account is out of quota: "You\'ve used all of your requests for this feature" — wait for the reset, or add another account';
            }
          })
          .catch(() => {});
      }
    });
    return () => quotaError;
  })();
  await page.goto(GEMINI_APP_URL, { waitUntil: "domcontentloaded", timeout: 20000 });
  return stopListening;
}

// Wait for the composer to appear; throws the friendliest available error.
// 30s: a freshly authenticated session loads the app shell slowly (~17s).
async function waitForComposer(page, getQuotaError, timeoutMs = 30000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if ((await page.locator(".ql-editor, [contenteditable='true']").count()) > 0) {
      return page.waitForSelector(".ql-editor, [contenteditable='true']", { timeout: 5000 });
    }
    const problem = await detectPageProblem(page, getQuotaError);
    if (problem) throw Error(problem);
    await page.waitForTimeout(400);
  }
  const quotaError = getQuotaError();
  if (quotaError) throw Error(quotaError);
  throw Error(`Gemini composer did not appear within ${Math.round(timeoutMs / 1000)}s (final URL: ${page.url()})`);
}

function nonEmpty(v) {
  if (typeof v !== "string") return "";
  const s = v.trim();
  return s.length > 0 ? s : "";
}

function firstNonEmpty(obj, keys) {
  if (!obj || typeof obj !== "object" || Array.isArray(obj)) return "";
  for (const key of keys) {
    const value = nonEmpty(obj[key]);
    if (value) return value;
  }
  return "";
}

// Collect the Gemini cookies from a connection record: apiKey/cookie field,
// providerSpecificData.cookie, or separate __Secure-1PSID(+TS) fields.
function extractGeminiCookies(credentials) {
  const direct = nonEmpty(credentials?.apiKey) || nonEmpty(credentials?.cookie);
  if (direct) return normalizeCookieInput(direct);
  const psd = credentials?.providerSpecificData;
  const cookie = firstNonEmpty(psd, ["cookie"]);
  if (cookie) return normalizeCookieInput(cookie);
  const psid = firstNonEmpty(psd, ["__Secure-1PSID"]);
  const psidts = firstNonEmpty(psd, ["__Secure-1PSIDTS"]);
  return [
    psid ? normalizeCookieInput(psid, "__Secure-1PSID") : "",
    psidts ? normalizeCookieInput(psidts, "__Secure-1PSIDTS") : "",
  ]
    .filter(Boolean)
    .join("; ");
}

// Flatten an OpenAI messages array into one prompt the web composer can take.
function flattenMessages(messages) {
  const usable = messages.filter((m) => typeof m.content === "string" && m.content.trim().length > 0);
  const userMessages = usable.filter((m) => m.role === "user");
  const lastUser = userMessages[userMessages.length - 1];
  const lastContent = lastUser?.content ?? "";
  const lastUserIndex = lastUser ? usable.lastIndexOf(lastUser) : -1;
  const history = usable.filter((m, i) => i < lastUserIndex && (m.role === "user" || m.role === "assistant"));
  const system = usable
    .filter((m) => m.role === "system")
    .map((m) => m.content)
    .join("\n\n");
  if (history.length === 0 && !system) return lastContent;
  if (history.length === 0) return `System:\n${system}\n\n${lastContent}`;
  const parts = [];
  if (system) parts.push(`System:\n${system}`);
  parts.push(`Previous conversation:\n${history.map((m) => `${m.role === "assistant" ? "Assistant" : "User"}: ${m.content}`).join("\n\n")}`);
  parts.push(`Current user message:\n${lastContent}`);
  return parts.join("\n\n");
}

async function sseResponseFromText(text, tools, isStream, model, cid, created) {
  const response = new Response(JSON.stringify(buildNonStreamResponse(text, model)), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
  // Tool-calls conversion is not supported here: tools are rejected earlier,
  // so the response always flows through the plain text paths below.
  void tools;
  void cid;
  void created;
  if (isStream) {
    const encoder = new TextEncoder();
    const stream = new ReadableStream({
      start(controller) {
        controller.enqueue(encoder.encode(`data: ${JSON.stringify(buildStreamChunk(text, model, "stop"))}\n\n`));
        controller.enqueue(encoder.encode(SSE_DONE));
        controller.close();
      },
    });
    return new Response(stream, { status: 200, headers: { ...SSE_HEADERS_NO_BUFFER } });
  }
  return response;
}

export class GeminiWebExecutor extends BaseExecutor {
  constructor() {
    super("gemini-web", {
      id: "gemini-web",
      baseUrl: GEMINI_APP_URL,
    });
  }

  async testConnection(credentials) {
    try {
      const cookies = extractGeminiCookies(credentials);
      if (!cookies) return false;
      return cookies
        .split(";")
        .map((part) => part.trim())
        .filter(Boolean)
        .map((pair) => {
          const eq = pair.indexOf("=");
          if (eq === -1) return null;
          const name = pair.substring(0, eq).trim();
          const value = pair.substring(eq + 1).trim();
          if (!name || !value || ["path", "domain", "expires", "max-age", "secure", "httponly", "samesite"].includes(name.toLowerCase())) {
            return null;
          }
          return { name, value };
        })
        .filter(Boolean)
        .some((cookie) => cookie.value.length > 0);
    } catch {
      return false;
    }
  }

  async execute({ model, body, stream, credentials, signal, log }) {
    const input = body || {};
    const tools = input.tools;
    const legacyFunctions = input.functions;
    if (tools != null && (!Array.isArray(tools) || tools.length > 0)) {
      return { response: errorResponse(400, "Gemini Web does not support OpenAI function tools"), url: GEMINI_APP_URL, headers: {}, transformedBody: body };
    }
    if (legacyFunctions != null && (!Array.isArray(legacyFunctions) || legacyFunctions.length > 0)) {
      return { response: errorResponse(400, "Gemini Web does not support legacy function tools"), url: GEMINI_APP_URL, headers: {}, transformedBody: body };
    }
    const rawEffort = typeof input.reasoning_effort === "string" && input.reasoning_effort.trim().length > 0 ? input.reasoning_effort.trim().toLowerCase() : null;
    const wantsThinking = rawEffort !== null && !NO_THINKING_EFFORTS.has(rawEffort);
    const cookieHeader = extractGeminiCookies(credentials);
    if (!cookieHeader && !cdpEndpoint()) {
      return { response: errorResponse(401, "Missing Gemini cookies (or set GEMINI_WEB_CDP_ENDPOINT to attach to a logged-in Chrome)"), url: GEMINI_APP_URL, headers: {}, transformedBody: body };
    }
    const prompt = flattenMessages(input.messages || []);
    const hasUserMessage = (input.messages || []).some(
      (m) => m.role === "user" && typeof m.content === "string" && m.content.trim().length > 0
    );
    if (!prompt || !hasUserMessage) {
      return { response: errorResponse(400, "No user message found"), url: GEMINI_APP_URL, headers: {}, transformedBody: body };
    }

    const requestedModel = model || "gemini-2.5-pro";
    let session = null;
    let removeAbortHandler = null;
    try {
      if (signal?.aborted) throw signal.reason instanceof Error ? signal.reason : Error("Request aborted");
      session = await acquireSession(cookieHeader);
      const { browser, context, page, owned } = session;
      const closePage = async () => {
        await page.close().catch(() => {});
        if (owned) await context.close().catch(() => {});
      };
      removeAbortHandler = () => {
        void closePage();
      };
      signal?.addEventListener("abort", removeAbortHandler, { once: true });
      const getQuotaError = await openGeminiApp(page);
      if (signal?.aborted) throw signal.reason instanceof Error ? signal.reason : Error("Request aborted");
      await waitForComposer(page, getQuotaError);
      await page.waitForTimeout(2000); // let the mode controls hydrate
      if (model) {
        const switched = await confirmModelMode(page, requestedModel, 5000);
        if (!switched.confirmed) {
          log?.warn?.("GEMINI-WEB", `Rejected request: could not confirm Gemini UI mode for "${requestedModel}" (${switched.reason})`);
          return {
            response: errorResponse(400, {
              message: unsupportedControlMessage(requestedModel, switched.reason) + (switched.indicatorText ? ` [indicator read: "${switched.indicatorText}"]` : ""),
              type: "invalid_request_error",
              code: UNSUPPORTED_CONTROL_CODE,
            }),
            url: GEMINI_APP_URL,
            headers: {},
            transformedBody: body,
          };
        }
      }
      if (wantsThinking) {
        const thinking = await confirmControl(page, EXTENDED_THINKING_DESCRIPTOR, 5000);
        if (!thinking.confirmed) {
          log?.warn?.("GEMINI-WEB", `Rejected request: could not confirm Extended Thinking is available (${thinking.reason})`);
          return {
            response: errorResponse(400, {
              message: unsupportedControlMessage("Extended Thinking", thinking.reason),
              type: "invalid_request_error",
              code: UNSUPPORTED_CONTROL_CODE,
            }),
            url: GEMINI_APP_URL,
            headers: {},
            transformedBody: body,
          };
        }
      }
      const composer = await waitForComposer(page, getQuotaError);
      await composer.click();
      await page.keyboard.insertText(prompt);
      await page.waitForTimeout(300);
      await page.keyboard.press("Enter");
      const baseState = await readChatState(page);
      if (stream) {
        const encoder = new TextEncoder();
        const pageRef = page;
        const sessionRef = session;
        session = null;
        const out = new ReadableStream(
          {
            async start(controller) {
              try {
                await pollTextReply({
                  page: pageRef,
                  baseCount: baseState.count,
                  baseText: baseState.lastText,
                  signal,
                  onDelta: (delta) => {
                    controller.enqueue(encoder.encode(`data: ${JSON.stringify(buildStreamChunk(delta, requestedModel))}\n\n`));
                  },
                });
                controller.enqueue(encoder.encode(`data: ${JSON.stringify(buildStreamChunk("", requestedModel, "stop"))}\n\n`));
                controller.enqueue(encoder.encode(SSE_DONE));
                controller.close();
              } catch (err) {
                controller.enqueue(
                  encoder.encode(`data: ${JSON.stringify({ error: { message: err.message || "gemini-web stream failed" } })}\n\n`)
                );
                controller.enqueue(encoder.encode(SSE_DONE));
                controller.close();
              } finally {
                await pageRef.close().catch(() => {});
                if (sessionRef.owned) await sessionRef.context.close().catch(() => {});
                await sessionRef.browser.close().catch(() => {});
              }
            },
          },
          { highWaterMark: 16384 }
        );
        return { response: new Response(out, { status: 200, headers: { ...SSE_HEADERS_NO_BUFFER } }), url: GEMINI_APP_URL, headers: {}, transformedBody: body };
      }
      const text = await pollTextReply({
        page,
        baseCount: baseState.count,
        baseText: baseState.lastText,
        signal,
      });
      return {
        response: new Response(JSON.stringify(buildNonStreamResponse(text, requestedModel)), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
        url: GEMINI_APP_URL,
        headers: {},
        transformedBody: body,
      };
    } catch (err) {
      const message = err instanceof Error ? err.message : "Unknown error";
      if (isBrowserMissingError(message)) {
        return {
          response: errorResponse(
            503,
            "Gemini Web needs a Chrome/Chromium binary. Install Google Chrome, set NINEROUTER_BROWSER_BIN or CHROME_PATH to an existing Chrome, or run `npx playwright-core install chromium`."
          ),
          url: GEMINI_APP_URL,
          headers: {},
          transformedBody: body,
        };
      }
      if (
        err instanceof Error &&
        (err.name === "TimeoutError" || message.includes("waitForSelector") || message.includes("Timeout") || message.includes("actionability") || message.includes("interception"))
      ) {
        return { response: errorResponse(400, message), url: GEMINI_APP_URL, headers: {}, transformedBody: body };
      }
      return { response: errorResponse(500, message), url: GEMINI_APP_URL, headers: {}, transformedBody: body };
    } finally {
      if (removeAbortHandler) signal?.removeEventListener("abort", removeAbortHandler);
      if (session) {
        await session.page.close().catch(() => {});
        if (session.owned) await session.context.close().catch(() => {});
        // For CDP sessions close() only disconnects — the user's Chrome stays.
        await session.browser.close().catch(() => {});
      }
    }
  }

  async generateMedia({ prompt, n = 1, size, credentials, signal, log }) {
    void log;
    if (!prompt) throw Error("prompt is required for image generation");
    const cookieHeader = extractGeminiCookies(credentials);
    if (!cookieHeader && !cdpEndpoint()) throw Error("Missing Gemini cookies");
    const images = [];
    for (let i = 0; i < Math.max(1, n); i++) {
      let session = null;
      try {
        session = await acquireSession(cookieHeader);
        const page = session.page;
        const getQuotaError = await openGeminiApp(page);
        const composer = await waitForComposer(page, getQuotaError);
        await composer.click();
        const aspect = size ? ` (aspect ratio ${size})` : "";
        await page.keyboard.insertText(`Generate an image${aspect}: ${prompt}`);
        await page.waitForTimeout(300);
        await page.keyboard.press("Enter");
        const image = await pollImageReply(page, { signal });
        images.push(image);
      } finally {
        if (session) {
          await session.page.close().catch(() => {});
          if (session.owned) await session.context.close().catch(() => {});
          await session.browser.close().catch(() => {});
        }
      }
    }
    return images;
  }
}
