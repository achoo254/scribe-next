<script setup lang="ts">
// Header shell OneScribe (web) — khớp desktop os-header: BrandMark + tên brand (primary).
// Web thêm email + Đăng xuất bên phải (desktop không có auth).
import { ViButton, ViIcon } from "@hobui/viui";
import { useAuth } from "../composables/use-auth";
import BrandMark from "./BrandMark.vue";

const { user, logout } = useAuth();

async function onLogout(): Promise<void> {
  await logout();
  // Guard router sẽ đẩy về /login khi user=null ở lần điều hướng kế; ép reload về gốc cho chắc.
  window.location.assign("/login");
}
</script>

<template>
  <header class="os-header">
    <div class="os-brand">
      <BrandMark :size="28" />
      <span class="os-brand-name">OneScribe</span>
    </div>
    <div class="os-header-right">
      <span v-if="user" class="os-email">{{ user.email }}</span>
      <ViButton variant="text" color="primary" size="small" @click="onLogout">
        <ViIcon icon="logout" start />Đăng xuất
      </ViButton>
    </div>
  </header>
</template>

<style scoped>
.os-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}
.os-brand {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}
.os-brand-name {
  font-size: 1.125rem;
  font-weight: 600;
  color: rgb(var(--v-theme-primary));
}
.os-header-right {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}
.os-email {
  color: rgb(var(--v-theme-on-surface));
  opacity: 0.7;
  font-size: 0.875rem;
}
</style>
