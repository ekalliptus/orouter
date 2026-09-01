// Login page — POST /api/auth/login sets the httpOnly auth_token cookie,
// then we refresh the cached auth flag BEFORE navigating (RequireAuth reads
// the cache; a stale false would bounce us straight back here).
<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { fetchAuthStatus, toast } from "@/lib/state";
import Logo from "@/components/Logo.vue";

const router = useRouter();
const password = ref("");
const error = ref<string | null>(null);
const loading = ref(false);

async function handleSubmit() {
  if (!password.value.trim() || loading.value) return;
  error.value = null;
  loading.value = true;
  try {
    const res = await fetch("/api/auth/login", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      credentials: "include",
      body: JSON.stringify({ password: password.value }),
    });
    const data = (await res.json()) as { success?: boolean; error?: string };
    if (!res.ok || !data.success) {
      error.value = data.error ?? "Login failed";
      return;
    }
    await fetchAuthStatus(true);
    toast.success("Welcome to your crayon router!", "Let's draw");
    router.push("/dashboard");
  } catch {
    error.value = "Could not reach the backend. Is the Rust server running?";
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <div class="fade-in" style="min-height: 100vh; display: flex; align-items: center; justify-content: center; padding: 1.5rem">
    <form
      class="kid-card kid-wobble-strong"
      style="width: min(440px, 100%); background: var(--color-surface)"
      @submit.prevent="handleSubmit"
    >
      <div style="text-align: center; margin-bottom: 1.5rem">
        <div style="display: flex; justify-content: center; margin-bottom: 0.5rem">
          <Logo :size="44" />
        </div>
        <h1 style="font-size: 2rem; margin: 0.25rem 0">ORouter</h1>
        <p style="font-family: var(--font-body); color: var(--color-text-muted); margin: 0">Draw your way to every AI</p>
      </div>

      <div
        v-if="error"
        class="kid-card"
        style="background: var(--color-danger); color: #fff; padding: 0.7rem 0.9rem; margin-bottom: 1rem; box-shadow: var(--nb-shadow-sm)"
      >
        {{ error }}
      </div>

      <label for="password" style="display: block; font-family: var(--font-body); font-size: 1.05rem; margin-bottom: 0.4rem">Secret password</label>
      <input
        id="password"
        v-model="password"
        type="password"
        class="kid-input"
        placeholder="type your secret..."
        autocomplete="current-password"
        :disabled="loading"
      />

      <button type="submit" class="kid-btn kid-btn--primary" style="width: 100%; margin-top: 1.25rem; font-size: 1.05rem" :disabled="loading || !password">
        {{ loading ? "Drawing..." : "Open my drawing" }}
      </button>

      <p style="font-family: var(--font-body); font-size: 0.95rem; color: var(--color-text-subtle); text-align: center; margin-top: 1rem">
        Default password is <code>123456</code> until you set one
      </p>
    </form>
  </div>
</template>
