/* Copied from localman-next apps/web/src/components/layout/idp-account-items.tsx.
   Scribe change: two entries, the account page and its authorised apps,
   on the build-time IdP origin (lib/idp.ts) instead of `tenant-info`
   for the links (the name comes from `useIdpName`, `GET /api/idp`); no
   feature flag and no heading (the entry names the account itself). */
import { IconExternalLink, IconPlug, IconUser } from "@tabler/icons-react";
import { DropdownMenuItem } from "../components/ui/dropdown-menu";
import { useT } from "../i18n/i18n-context";
import { openExternal } from "../lib/external-link";
import { IDP_ACCOUNT_URL, IDP_APPS_URL } from "../lib/idp";
import { useIdpName } from "../lib/idp-name";

/** The account menu's links to the IdP account pages, each in a new tab. */
export function IdpAccountItems() {
	const t = useT();
	const idpName = useIdpName();
	const entries = [
		{
			id: "account",
			label: t("layout.idpMenu.account", { idpName }),
			url: IDP_ACCOUNT_URL,
			Icon: IconUser,
		},
		{
			id: "apps",
			label: t("layout.idpMenu.apps"),
			url: IDP_APPS_URL,
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
