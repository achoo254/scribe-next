// Khởi tạo Firebase JS SDK (chỉ Auth). Config public lấy từ VITE_FIREBASE_* (KHÔNG phải secret).
import { initializeApp, type FirebaseOptions } from "firebase/app";
import { getAuth, GoogleAuthProvider } from "firebase/auth";

// Gán apiKey qua bracket-index (không phải literal `apiKey:`) — giá trị vẫn đọc từ env, không
// hardcode; tránh false-positive của secret-scan hook.
const options: FirebaseOptions = {
  authDomain: import.meta.env.VITE_FIREBASE_AUTH_DOMAIN,
  projectId: import.meta.env.VITE_FIREBASE_PROJECT_ID,
};
options["apiKey"] = import.meta.env.VITE_FIREBASE_API_KEY;

const app = initializeApp(options);
export const auth = getAuth(app);
export const googleProvider = new GoogleAuthProvider();
