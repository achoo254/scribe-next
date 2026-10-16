import { useRef } from "react";
import { useBlocker } from "react-router";
import { ConfirmModal } from "../components/ui/confirm-modal";
import { useT } from "../i18n/i18n-context";

export interface LeaveGuardProps {
	/** Something typed is not saved yet: leaving the page would drop it. */
	when: boolean;
	"data-id"?: string;
}

/**
 * Asks before an in-app navigation (nav, back button, swipe back in the
 * iOS app) leaves a page with unsaved input. Closing or reloading the tab
 * is not covered: WKWebView never fires `beforeunload`.
 */
export function LeaveGuard({ when, "data-id": dataId }: LeaveGuardProps) {
	const t = useT();
	const blocker = useBlocker(when);
	// After `proceed()` the page unmounts while the modal still reports
	// "closed"; a `reset()` then would hit a blocker that is gone.
	const proceeding = useRef(false);

	return (
		<ConfirmModal
			open={blocker.state === "blocked"}
			onOpenChange={(open) => {
				if (!open && !proceeding.current) blocker.reset?.();
			}}
			title={t("layout.leaveGuard.title")}
			question={t("layout.leaveGuard.question")}
			confirmLabel={t("layout.leaveGuard.confirm")}
			onConfirm={async () => {
				proceeding.current = true;
				blocker.proceed?.();
			}}
			data-id={dataId}
		>
			{t("layout.leaveGuard.detail")}
		</ConfirmModal>
	);
}
