/// <reference types="vite/client" />

interface ImportMetaEnv {
	readonly VITE_NATIVE_API_ORIGIN?: string;
	/** hoandat-auth origin for the account links (src/lib/idp.ts). */
	readonly VITE_IDP_ORIGIN?: string;
	/** Support email on the legal pages (src/lib/legal.ts); empty: no email. */
	readonly VITE_SUPPORT_EMAIL?: string;
}

interface ImportMeta {
	readonly env: ImportMetaEnv;
}
