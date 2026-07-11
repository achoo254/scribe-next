<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { useAuth } from "../composables/use-auth";

const { loginWithGoogle } = useAuth();
const router = useRouter();
const error = ref("");
const loading = ref(false);

async function onLogin(): Promise<void> {
  error.value = "";
  loading.value = true;
  try {
    await loginWithGoogle();
    await router.push({ name: "home" });
  } catch (e) {
    error.value = e instanceof Error ? e.message : "Đăng nhập thất bại";
  } finally {
    loading.value = false;
  }
}
</script>

<template>
  <main class="login">
    <div class="card">
      <h1>OneScribe</h1>
      <p class="sub">Ghi &amp; cắt media</p>
      <button class="google" :disabled="loading" @click="onLogin">
        {{ loading ? "Đang đăng nhập…" : "Đăng nhập với Google" }}
      </button>
      <p v-if="error" class="err">{{ error }}</p>
    </div>
  </main>
</template>

<style scoped>
.login {
  min-height: 100vh;
  display: grid;
  place-items: center;
}
.card {
  text-align: center;
  padding: 2.5rem;
  border: 1px solid #e5e7eb;
  border-radius: 12px;
  min-width: 320px;
}
h1 {
  margin: 0 0 0.25rem;
}
.sub {
  color: #6b7280;
  margin: 0 0 1.5rem;
}
.google {
  width: 100%;
  padding: 0.7rem 1rem;
  font-size: 1rem;
  cursor: pointer;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  background: #fff;
}
.google:disabled {
  opacity: 0.6;
  cursor: default;
}
.err {
  color: #dc2626;
  margin-top: 1rem;
}
</style>
