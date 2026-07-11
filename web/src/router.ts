import { createRouter, createWebHistory } from "vue-router";
import { useAuth } from "./composables/use-auth";
import HomeView from "./views/HomeView.vue";
import LoginView from "./views/LoginView.vue";

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", name: "home", component: HomeView, meta: { requiresAuth: true } },
    { path: "/login", name: "login", component: LoginView },
  ],
});

// Guard: session là httpOnly cookie (JS không đọc được) → xác thực bằng /api/me (gọi 1 lần).
router.beforeEach(async (to) => {
  const { user, ready, fetchMe } = useAuth();
  if (!ready.value) {
    await fetchMe();
  }
  if (to.meta.requiresAuth && !user.value) {
    return { name: "login" };
  }
  if (to.name === "login" && user.value) {
    return { name: "home" };
  }
  return true;
});
