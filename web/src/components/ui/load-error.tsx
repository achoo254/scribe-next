import { useT } from "../../i18n/i18n-context";
import { errorMessage } from "../../lib/api";
import { Alert } from "./alert";
import { Button } from "./button";

export interface LoadErrorProps {
	/** What could not be loaded, e.g. "Không tải được lịch sử ghi". */
	title: string;
	error: unknown;
	onRetry: () => void;
	/** The retry is running: the button shows its spinner. */
	retrying?: boolean;
	"data-id"?: string;
}

/**
 * A list or document that failed to load: the reason in a danger alert and
 * a "Thử lại" button below it, the error state of localman-next's lists.
 */
export function LoadError({
	title,
	error,
	onRetry,
	retrying = false,
	"data-id": dataId,
}: LoadErrorProps) {
	const t = useT();
	return (
		<div data-id={dataId} className="grid justify-items-start gap-3">
			<Alert tone="danger" title={title}>
				{errorMessage(error)}
			</Alert>
			<Button
				data-id={dataId ? `${dataId}-retry` : undefined}
				loading={retrying}
				onClick={onRetry}
			>
				{t("common.actions.retry")}
			</Button>
		</div>
	);
}
