<script setup lang="ts">
import { useRouter } from "vue-router";
import { ViButton } from "@hobui/viui";
import { useAuth } from "../composables/use-auth";

const { user, logout } = useAuth();
const router = useRouter();

async function onLogout(): Promise<void> {
  await logout();
  await router.push({ name: "login" });
}
</script>

<template>
  <header class="topbar">
    <router-link to="/" class="brand">OneScribe</router-link>
    <div class="right">
      <span v-if="user" class="email">{{ user.email }}</span>
      <ViButton variant="text" color="primary" size="small" label="Đăng xuất" @click="onLogout" />
    </div>
  </header>
</template>

<style scoped>
.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-8pt-2) var(--space-8pt-3);
  background: #fff;
  border-bottom: 1px solid var(--bsg-nl-200, #e5e7eb);
}
.brand {
  color: var(--bsg-primary);
  text-decoration: none;
  font-weight: 700;
  font-size: 1.125rem;
}
.right {
  display: flex;
  align-items: center;
  gap: var(--space-8pt-2);
}
.email {
  color: var(--bsg-nl-600, #4a6278);
  font-size: 0.875rem;
}
</style>
