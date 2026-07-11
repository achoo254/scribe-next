<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { ViCard, ViButton, ViAlert } from "@hobui/viui";
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
  <div class="login-page">
    <ViCard class="login-card" elevation="3" rounded="lg">
      <div class="text-center">
        <h1 class="text-h4 font-weight-bold text-primary mb-1">OneScribe</h1>
        <p class="text-body-2 text-medium-emphasis mb-8">Ghi &amp; cắt media</p>
        <ViButton
          block
          color="primary"
          size="large"
          variant="flat"
          prepend-icon="tabler-brand-google"
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
      </div>
    </ViCard>
  </div>
</template>

<style scoped>
.login-page {
  min-height: 100dvh;
  display: grid;
  place-items: center;
  padding: var(--space-8pt-2);
  background: var(--bsg-nl-50, #f6f8fa);
}
.login-card {
  width: 100%;
  max-width: 380px;
  padding: var(--space-8pt-6) var(--space-8pt-4);
}
</style>
