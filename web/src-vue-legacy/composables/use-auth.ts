// Auth state + luồng Firebase Google sign-in ↔ session cookie BE.
//
// Firebase xử lý popup Google → trả ID token; ta ĐỔI ngay lấy session cookie httpOnly của BE
// (POST /api/auth/session) → sau đó BE cookie là nguồn xác thực (httpOnly, chống XSS).
import { ref } from "vue";
import { signInWithPopup, signOut } from "firebase/auth";
import { auth, googleProvider } from "../lib/firebase";

export interface CurrentUser {
  email: string;
}

const user = ref<CurrentUser | null>(null);
const ready = ref(false);

async function fetchMe(): Promise<void> {
  try {
    const res = await fetch("/api/me", { credentials: "include" });
    user.value = res.ok ? ((await res.json()) as CurrentUser) : null;
  } catch {
    user.value = null;
  } finally {
    ready.value = true;
  }
}

async function loginWithGoogle(): Promise<void> {
  const cred = await signInWithPopup(auth, googleProvider);
  const idToken = await cred.user.getIdToken();
  const res = await fetch("/api/auth/session", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    credentials: "include",
    body: JSON.stringify({ idToken }),
  });
  if (!res.ok) {
    // BE từ chối (allowlist / token sai) → dọn Firebase session để không kẹt trạng thái nửa vời.
    await signOut(auth).catch(() => undefined);
    const body = (await res.json().catch(() => null)) as { error?: string } | null;
    throw new Error(body?.error ?? "Đăng nhập thất bại");
  }
  user.value = (await res.json()) as CurrentUser;
}

async function logout(): Promise<void> {
  await fetch("/api/auth/logout", { method: "POST", credentials: "include" }).catch(() => undefined);
  await signOut(auth).catch(() => undefined);
  user.value = null;
}

export function useAuth() {
  return { user, ready, fetchMe, loginWithGoogle, logout };
}
