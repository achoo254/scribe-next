/*
 * Browser view of web/brand.config.json (pattern of chat-next
 * apps/web/src/brand.ts @a5be0b4). brand/src reads the file with node:fs
 * and validates it against the schema; the app imports the JSON through Vite,
 * and the brand tests keep the file valid.
 */
import brandConfig from "../../brand.config.json";

/** Product name shown in the UI (the wordmark); never hard-code it. */
export const APP_NAME = brandConfig.appName;

/** Issuer of the primary tenant: the default IdP origin. */
export const PRIMARY_ISSUER = brandConfig.primaryTenant.issuer;
