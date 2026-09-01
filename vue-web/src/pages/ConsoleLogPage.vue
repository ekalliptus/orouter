// Console Log: live server log tail over SSE + snapshot history + clear.
<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { api } from "@/lib/api";
import { toast } from "@/lib/state";

interface LogLine {
  ts: string;
  level: string;
  target: string;
  message: string;
}

const logs = ref<LogLine[]>([]);
const paused = ref(false);
const autoScroll = ref(true);
const connected = ref(false);
let es: EventSource | null = null;
const scroller = ref<HTMLElement | null>(null);

const levelColor: Record<string, string> = {
  ERROR: "var(--color-danger)",
  WARN: "#d97706",
  INFO: "var(--color-info)",
  DEBUG: "var(--color-text-subtle)",
  TRACE: "var(--color-text-subtle)",
};

function connect() {
  es?.close();
  es = new EventSource("/api/console-logs/stream", { withCredentials: true });
  es.onopen = () => {
    connected.value = true;
  };
  es.onmessage = (ev) => {
    if (paused.value) return;
    try {
      logs.value.push(JSON.parse(ev.data) as LogLine);
      if (logs.value.length > 2000) logs.value = logs.value.slice(-1500);
      if (autoScroll.value) {
        requestAnimationFrame(() => {
          scroller.value?.scrollTo({ top: scroller.value.scrollHeight });
        });
      }
    } catch { /* skip malformed line */ }
  };
  es.onerror = () => {
    connected.value = false;
    // EventSource retries on its own; keep the flag honest meanwhile.
  };
}

onMounted(connect);
onBeforeUnmount(() => es?.close());

async function clearLogs() {
  try {
    await api.del("/api/console-logs");
    logs.value = [];
    toast.success("Console cleared");
  } catch {
    toast.error("Failed to clear console");
  }
}
</script>

<template>
  <div class="fade-in flex flex-col gap-3" style="max-width: 1100px">
    <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.5rem">
      <div style="display: flex; gap: 0.5rem; align-items: center">
        <Badge :variant="connected ? 'success' : 'danger'" size="sm" dot>
          {{ connected ? "LIVE" : "DISCONNECTED" }}
        </Badge>
        <span style="font-family: var(--font-body); font-size: 0.9rem; color: var(--color-text-muted)">
          {{ logs.length }} line(s)
        </span>
      </div>
      <div style="display: flex; gap: 0.4rem">
        <button class="kid-btn" style="padding: 0.25rem 0.6rem; font-size: 0.82rem" @click="paused = !paused">
          <span class="material-symbols-outlined" style="font-size: 15px">{{ paused ? "play_arrow" : "pause" }}</span>
          {{ paused ? "Resume" : "Pause" }}
        </button>
        <button
          class="kid-btn"
          style="padding: 0.25rem 0.6rem; font-size: 0.82rem"
          :style="autoScroll ? { background: 'var(--color-brand-500)', color: '#fff' } : {}"
          @click="autoScroll = !autoScroll"
        >
          Auto-scroll
        </button>
        <button class="kid-btn" style="padding: 0.25rem 0.6rem; font-size: 0.82rem; background: var(--color-danger); color: #fff" @click="clearLogs">
          <span class="material-symbols-outlined" style="font-size: 15px">delete</span> Clear
        </button>
      </div>
    </div>

    <div
      ref="scroller"
      class="kid-card"
      style="padding: 0; overflow-y: auto; height: 65vh; font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-size: 0.78rem; background: var(--color-bg-alt)"
    >
      <div
        v-for="(l, i) in logs"
        :key="i"
        style="padding: 0.25rem 0.7rem; border-bottom: 1px solid color-mix(in srgb, var(--color-text-main) 6%, transparent); display: flex; gap: 0.7rem"
      >
        <span style="color: var(--color-text-subtle); white-space: nowrap">{{ l.ts }}</span>
        <span :style="{ color: levelColor[l.level] ?? 'var(--color-text-main)', fontWeight: 700, minWidth: '44px' }">{{ l.level }}</span>
        <span style="color: var(--color-text-muted); white-space: nowrap">{{ l.target }}</span>
        <span style="white-space: pre-wrap; word-break: break-word; color: var(--color-text-main)">{{ l.message }}</span>
      </div>
      <div v-if="logs.length === 0" style="padding: 2rem; text-align: center; font-family: var(--font-body)">
        Waiting for log events…
      </div>
    </div>
  </div>
</template>
