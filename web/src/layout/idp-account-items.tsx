/* Copied from localman-next apps/web/src/components/layout/idp-account-items.tsx.
   Scribe change: two entries, the account page and its authorised apps,
   at the IdP of this host's tenant (lib/idp.ts, lib/tenant-info.ts); no
   feature flag and no heading (the entry names the account itself). */
import { IconExternalLink, IconPlug, IconUser } from "@tabler/icons-react";
import { DropdownMenuItem } from "../components/ui/dropdown-menu";
import { useT } from "../i18n/i18n-context";
import { openExternal } from "../lib/external-link";
import { idpAccountUrlOf, idpAppsUrlOf } from "../lib/idp";
import { useTenantInfo } from "../lib/tenant-info";

/** The account menu's links to the IdP account pages, each in a new tab. */
export function IdpAccountItems() {
	const t = useT();
	const tenant = useTenantInfo();
	const idpName = tenant.displayName;
	const entries = [
		{
			id: "account",
			label: t("layout.idpMenu.account", { idpName }),
			url: idpAccountUrlOf(tenant),
			Icon: IconUser,
		},
		{
			id: "apps",
			label: t("layout.idpMenu.apps"),
			url: idpAppsUrlOf(tenant),
			Icon: IconPlug,
		},
	];
	return (
		<>
			{entries.map(({ id, label, url, Icon }) => (
				<DropdownMenuItem
					key={id}
					data-id={`account-menu-${id}`}
					icon={<Icon size={16} aria-hidden={true} />}
					onSelect={() => openExternal(url)}
				>
					{label}
					<span className="sr-only">
						{t("layout.idpMenu.opensIdp", { idpName })}
					</span>
					<IconExternalLink size={14} aria-hidden="true" className="ml-auto" />
				</DropdownMenuItem>
			))}
		</>
	);
}
