/*
 * Browser view of web/brand.config.json. defineBrand checks the required
 * fields without a schema validator (Ajv is blocked by the CSP); the full
 * schema runs at build time (pnpm brand:gen, vite-brand.ts, check:brand).
 */
import { defineBrand } from "@shared-next/brand";
import brandConfig from "../../brand.config.json";

const brand = defineBrand(brandConfig);

/** Product name shown in the UI (the wordmark); never hard-code it. */
export const APP_NAME = brand.appName;

/** Issuer of the primary tenant: the default IdP origin. */
export const PRIMARY_ISSUER = brand.primaryTenant.issuer;

/** Root domain of the primary tenant (`hoandat.com`), the only one Scribe runs on. */
export const PRIMARY_TENANT_DOMAIN = brand.primaryTenant.domain;

/** Home page of the ecosystem (`https://hoandat.com`), linked from the landing footer. */
export const ECOSYSTEM_HOME_URL = `https://${PRIMARY_TENANT_DOMAIN}`;
