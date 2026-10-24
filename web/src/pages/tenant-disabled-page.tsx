import { IconExternalLink } from "@tabler/icons-react";
import { BrandMark, Wordmark } from "../components/brand/wordmark";
import { ThemeToggle } from "../components/theme-toggle";
import { Card } from "../components/ui/card";
import { useT } from "../i18n/i18n-context";
import { renderRich } from "../i18n/rich-text";
import { TEXT_LINK } from "../layout/legal-links";
import { idpAdminUrlOf } from "../lib/idp";
import type { TenantInfo } from "../lib/tenant-info";

/**
 * "Scribe chưa bật trên domain này": what a domain whose owner has not
 * turned Scribe on shows instead of the sign-in page and every signed-in
 * page (no signed-in API is called). One sentence for its users, one for
 * its owner with a link to the `/admin` page of the domain's IdP, where
 * entering a transcription key turns Scribe on. Laid out like the sign-in
 * page. Rendered above the router (`TenantGate`), so links are plain anchors.
 */
export function TenantDisabledPage({ tenant }: { tenant: TenantInfo }) {
	const t = useT();
	const domain = tenant.domain;
	return (
		<div
			data-id="tenant-disabled"
			className="grid-backdrop grid min-h-dvh grid-rows-[auto_1fr] pb-[env(safe-area-inset-bottom)]"
		>
			<header className="border-border-subtle border-b bg-bg/82 pt-[env(safe-area-inset-top)] backdrop-blur-md">
				<div className="mx-auto flex min-h-(--topbar-h) w-full max-w-page items-center justify-between gap-3 px-gutter">
					<span className="flex items-center gap-2">
						<BrandMark />
						<Wordmark />
					</span>
					<ThemeToggle data-id="tenant-disabled-theme" />
				</div>
			</header>
			<main className="grid content-start justify-items-center px-gutter pt-12 pb-16">
				<Card className="grid w-full max-w-105 gap-4 px-6 py-8 sm:p-8">
					<div className="grid gap-1">
						<h1 className="m-0 font-bold font-mono text-[1.375rem] tracking-[-0.01em]">
							{t("tenant.disabled.title")}
						</h1>
						<p
							data-id="tenant-disabled-domain"
							className="m-0 font-mono text-muted text-sm"
						>
							{t("tenant.disabled.domain", { domain })}
						</p>
					</div>
					<p className="m-0 text-sm">
						{t("tenant.disabled.users", { domain })}
					</p>
					<p className="m-0 text-muted text-sm">
						{renderRich(t("tenant.disabled.owner", { domain }), {
							admin: (chunk) => (
								<a
									href={idpAdminUrlOf(tenant)}
									target="_blank"
									rel="noopener noreferrer"
									data-id="tenant-disabled-admin"
									className={`${TEXT_LINK} inline-flex items-center gap-1`}
								>
									{chunk}
									<IconExternalLink size={14} aria-hidden="true" />
								</a>
							),
						})}
					</p>
				</Card>
			</main>
		</div>
	);
}
