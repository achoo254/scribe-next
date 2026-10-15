import { createRouter, createWebHistory } from "vue-router";
import { useAuth } from "./composables/use-auth";
import HomeView from "./views/HomeView.vue";
import LoginView from "./views/LoginView.vue";

// Shell 1 trang (khớp desktop): "/" = shell 3 tab, Cắt file là tab (không còn route /split riêng).
export const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", name: "home", component: HomeView, meta: { requiresAuth: true } },
    { path: "/login", name: "login", component: LoginView },
    { path: "/:pathMatch(.*)*", redirect: { name: "home" } },
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
