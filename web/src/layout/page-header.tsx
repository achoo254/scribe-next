import type { ReactNode } from "react";
import { useT } from "../i18n/i18n-context";

export interface PageHeaderProps {
	/** Text of the page's h1. */
	title: ReactNode;
	/**
	 * Link or button back to the parent page: the title becomes the end of a
	 * breadcrumb ("Transcript / <name>"), as on localman-next's detail pages.
	 */
	parent?: ReactNode;
	/** One or two sentences under the title. */
	description?: ReactNode;
	/** Page-level actions at the end of the title row; they wrap on phones. */
	actions?: ReactNode;
	"data-id"?: string;
}

const TITLE_CLASS =
	"m-0 min-w-0 break-words font-bold font-mono text-xl tracking-[-0.01em]";

/**
 * Top of a signed-in page, following localman-next (`/projects`,
 * `/account`, a project's page): the h1 in bold mono on a 44px row, the
 * page's actions at its end, then a short muted description.
 */
export function PageHeader({
	title,
	parent,
	description,
	actions,
	"data-id": dataId,
}: PageHeaderProps) {
	const t = useT();
	return (
		<div data-id={dataId} className="grid gap-1">
			<div className="flex min-h-11 flex-wrap items-center justify-between gap-x-3 gap-y-2">
				{parent ? (
					<nav
						aria-label={t("layout.breadcrumb")}
						// Wraps on phones: a long title moves under "Transcript /"
						// instead of shrinking to one word per line.
						className="flex min-w-0 flex-1 flex-wrap items-baseline gap-x-2"
					>
						{parent}
						<span
							aria-hidden="true"
							className="font-mono text-icon-muted text-xl"
						>
							/
						</span>
						<h1 aria-current="page" className={TITLE_CLASS}>
							{title}
						</h1>
					</nav>
				) : (
					<h1 className={TITLE_CLASS}>{title}</h1>
				)}
				{actions ? (
					<div className="flex flex-wrap items-center gap-2">{actions}</div>
				) : null}
			</div>
			{description ? (
				<p className="m-0 text-muted text-sm">{description}</p>
			) : null}
		</div>
	);
}
