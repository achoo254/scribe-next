<script setup lang="ts">
import { useRouter } from "vue-router";
import { useAuth } from "../composables/use-auth";

const { user, logout } = useAuth();
const router = useRouter();

async function onLogout(): Promise<void> {
  await logout();
  await router.push({ name: "login" });
}
</script>

<template>
  <div class="app">
    <header class="topbar">
      <strong>OneScribe</strong>
      <div class="right">
        <span v-if="user" class="email">{{ user.email }}</span>
        <button @click="onLogout">Đăng xuất</button>
      </div>
    </header>
    <main class="content">
      <p>Đăng nhập thành công. Chức năng ghi media sẽ bổ sung ở các phase sau.</p>
      <router-link class="action" to="/split">Cắt file</router-link>
    </main>
  </div>
</template>

<style scoped>
.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1.25rem;
  border-bottom: 1px solid #e5e7eb;
}
.right {
  display: flex;
  align-items: center;
  gap: 1rem;
}
.email {
  color: #6b7280;
}
.content {
  padding: 2rem 1.25rem;
}
.action {
  display: inline-block;
  margin-top: 1rem;
  padding: 0.6rem 1.25rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  background: #2563eb;
  color: #fff;
  text-decoration: none;
}
</style>
