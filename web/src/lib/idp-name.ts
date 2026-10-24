/* The sign-in account's name, read with the tenant (`lib/tenant-info.ts`,
   one `GET /api/idp` per page load, started in `main.tsx`). */
import { useTenantInfo } from "./tenant-info";

export {
	DEFAULT_IDP_NAME,
	idpNameOf,
	loadTenantInfo as loadIdpName,
} from "./tenant-info";

/** The sign-in account's name: the default until the backend answers. */
export function useIdpName(): string {
	return useTenantInfo().displayName;
}
