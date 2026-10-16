import { IconAlertTriangle } from "@tabler/icons-react";
import { useRouteError } from "react-router";
import { Button } from "../components/ui/button";
import { EmptyState } from "../components/ui/empty-state";
import { useT } from "../i18n/i18n-context";

/**
 * Error boundary of the signed-in pages. It replaces only the page below
 * the shell, so `SignedInState` (a running recording, an upload) stays
 * mounted when one page fails to render.
 */
export function PageError() {
	const t = useT();
	const error = useRouteError();
	console.error("page render failed", error);
	return (
		<EmptyState
			data-id="page-error"
			icon={<IconAlertTriangle size={28} aria-hidden="true" />}
			title={t("layout.pageError.title")}
			action={
				<Button variant="secondary" onClick={() => window.location.reload()}>
					{t("layout.pageError.reload")}
				</Button>
			}
		>
			{t("layout.pageError.body")}
		</EmptyState>
	);
}
