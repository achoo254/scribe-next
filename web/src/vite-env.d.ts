/// <reference types="vite/client" />

interface ImportMetaEnv {
	readonly VITE_NATIVE_API_ORIGIN?: string;
	/** hoandat-auth origin for the account links (src/lib/idp.ts). */
	readonly VITE_IDP_ORIGIN?: string;
}

interface ImportMeta {
	readonly env: ImportMetaEnv;
}
