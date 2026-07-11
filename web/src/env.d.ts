/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_FIREBASE_API_KEY: string;
  readonly VITE_FIREBASE_AUTH_DOMAIN: string;
  readonly VITE_FIREBASE_PROJECT_ID: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<Record<string, never>, Record<string, never>, unknown>;
  export default component;
}

// Iconset tabler của ViUI: runtime export `tabler` nhưng .d.ts rỗng → khai báo cho TS.
declare module "@hobui/viui/iconsets/tabler" {
  import type { IconSet } from "vuetify";
  export const tabler: IconSet;
  export const tablerIconMap: Record<string, string>;
}
declare module "@hobui/viui/style.css";
