<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { ViCard, ViButton, ViAlert } from "@hobui/viui";
import { useAuth } from "../composables/use-auth";
import BrandMark from "../components/BrandMark.vue";

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
  <div class="login-page">
    <ViCard class="login-card" elevation="3" rounded="lg">
      <div class="login-brand">
        <BrandMark :size="44" />
        <h1 class="login-title">OneScribe</h1>
        <p class="login-sub">Ghi &amp; cắt media</p>
      </div>
      <ViButton
        block
        color="primary"
        size="large"
        variant="flat"
        prepend-icon="brand-google"
        :loading="loading"
        label="Đăng nhập với Google"
        @click="onLogin"
      />
      <ViAlert
        v-if="error"
        type="error"
        variant="tonal"
        density="compact"
        class="mt-4 text-left"
        :text="error"
      />
    </ViCard>
  </div>
</template>

<style scoped>
.login-page {
  min-height: 100dvh;
  display: grid;
  place-items: center;
  padding: 1rem;
  background: rgb(var(--v-theme-background));
}
.login-card {
  width: 100%;
  max-width: 380px;
  padding: 2.5rem 2rem;
}
.login-brand {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.25rem;
  margin-bottom: 2rem;
}
.login-title {
  margin: 0.5rem 0 0;
  font-size: 1.5rem;
  font-weight: 700;
  color: rgb(var(--v-theme-primary));
}
.login-sub {
  margin: 0;
  font-size: 0.875rem;
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.7;
}
</style>
