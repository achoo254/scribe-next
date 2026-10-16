import { IconSearch } from "@tabler/icons-react";
import { useEffect, useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { cardClassName } from "../../components/ui/card";
import { EmptyState } from "../../components/ui/empty-state";
import { Input } from "../../components/ui/input";
import { LoadError } from "../../components/ui/load-error";
import { PanelProgress } from "../../components/ui/panel-progress";
import { highlightParts, queryWords } from "../../features/search/normalize";
import type {
	AskCitation,
	SearchResult,
} from "../../features/search/search-api";
import {
	isSearchable,
	useSearchQuery,
} from "../../features/search/search-queries";
import { useT } from "../../i18n/i18n-context";
import { PATHS, transcriptPath } from "../../layout/app-paths";
import { PageHeader } from "../../layout/page-header";
import { cn } from "../../lib/cn";
import { formatClock, formatDateTime } from "../../lib/format";
import { AskCard, citationTitle } from "./ask-card";

/** Wait after the last key press before searching. */
const DEBOUNCE_MS = 300;

/** Same hover and focus look as the rows of "Lời đã chép". */
const ROW_ACTIVE =
	"transition-[border-color,box-shadow] " +
	"hover:border-[color-mix(in_srgb,var(--c-primary)_45%,var(--c-border))] hover:shadow-md " +
	"has-[:focus-visible]:border-[color-mix(in_srgb,var(--c-primary)_45%,var(--c-border))]";

/**
 * `/search?q=`: sentences of every transcript matching what is typed (a
 * short pause after typing, the URL keeps the query), the match marked; a
 * tap opens the reader at that second. "Hỏi Scribe" answers questions on
 * every transcript.
 */
export function SearchPage() {
	const t = useT();
	const navigate = useNavigate();
	const [params, setParams] = useSearchParams();
	const query = params.get("q") ?? "";
	const [draft, setDraft] = useState(query);
	const typed = useRef(query);

	// Back/forward changes the URL: show its query in the field.
	useEffect(() => {
		if (query !== typed.current) {
			typed.current = query;
			setDraft(query);
		}
	}, [query]);

	useEffect(() => {
		if (draft === typed.current) return;
		const timer = window.setTimeout(() => {
			typed.current = draft;
			setParams(draft.trim() ? { q: draft } : {}, { replace: true });
		}, DEBOUNCE_MS);
		return () => window.clearTimeout(timer);
	}, [draft, setParams]);

	const open = (key: string, at: number) => navigate(transcriptPath(key, at));

	return (
		<section data-id="search" className="grid gap-5">
			<PageHeader
				parent={
					<button
						type="button"
						data-id="search-back"
						className="shrink-0 cursor-pointer rounded-sm border-0 bg-transparent p-0 font-bold font-mono text-muted text-xl hover:text-primary pointer-coarse:min-h-11"
						onClick={() => navigate(PATHS.transcripts)}
					>
						{t("search.page.back")}
					</button>
				}
				title={t("search.page.title")}
				description={t("search.page.description")}
			/>
			<form
				className="relative"
				onSubmit={(event) => {
					event.preventDefault();
					typed.current = draft;
					setParams(draft.trim() ? { q: draft } : {}, { replace: true });
				}}
			>
				<IconSearch
					size={18}
					aria-hidden="true"
					className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-icon-muted"
				/>
				<Input
					type="search"
					data-id="search-input"
					aria-label={t("search.page.field")}
					placeholder={t("search.page.placeholder")}
					value={draft}
					maxLength={200}
					// Opened to search (from "Lời đã chép" with nothing typed yet): type at once.
					autoFocus={!query}
					className="pl-10"
					onChange={(event) => setDraft(event.target.value)}
				/>
			</form>
			<SearchResults query={query} onOpen={open} />
			<AskCard
				onCite={(citation: AskCitation) => open(citation.key, citation.at)}
			/>
		</section>
	);
}

function SearchResults({
	query,
	onOpen,
}: {
	query: string;
	onOpen: (key: string, at: number) => void;
}) {
	const t = useT();
	const search = useSearchQuery(query);
	if (queryWords(query).length === 0) {
		return (
			<p className="m-0 text-muted text-sm" data-id="search-idle">
				{t("search.results.idle")}
			</p>
		);
	}
	if (!isSearchable(query)) {
		return (
			<p className="m-0 text-muted text-sm" data-id="search-too-short">
				{t("search.results.tooShort")}
			</p>
		);
	}
	if (search.isPending) {
		return (
			<p className="m-0 text-muted text-sm" aria-live="polite">
				{t("search.results.searching")}
			</p>
		);
	}
	if (search.isError) {
		return (
			<LoadError
				data-id="search-error"
				title={t("search.results.failed")}
				error={search.error}
				retrying={search.isFetching}
				onRetry={() => void search.refetch()}
			/>
		);
	}
	// While the next query loads, the results (and their marks) are still those of the previous one.
	const { results, total, query: answered } = search.data;
	if (results.length === 0) {
		return (
			<EmptyState
				data-id="search-empty"
				icon={<IconSearch size={28} aria-hidden="true" />}
				title={t("search.results.emptyTitle")}
			>
				{t("search.results.emptyBody")}
			</EmptyState>
		);
	}
	return (
		<section className="grid gap-3" aria-labelledby="search-results-title">
			<div className="flex flex-wrap items-baseline justify-between gap-2">
				<h2
					id="search-results-title"
					className="m-0 font-mono font-semibold text-base"
				>
					{t("search.results.title")}
				</h2>
				<p
					className="m-0 text-muted text-sm"
					data-id="search-count"
					aria-live="polite"
				>
					{results.length < total
						? t("search.results.showing", { count: results.length, total })
						: t("search.results.count", { count: total })}
				</p>
			</div>
			<PanelProgress active={search.isFetching} aria-busy={search.isFetching}>
				<ul className="m-0 grid list-none gap-3 p-0">
					{results.map((result) => (
						<ResultRow
							key={`${result.key}@${result.at}`}
							result={result}
							query={answered}
							onOpen={onOpen}
						/>
					))}
				</ul>
			</PanelProgress>
		</section>
	);
}

function ResultRow({
	result,
	query,
	onOpen,
}: {
	result: SearchResult;
	query: string;
	onOpen: (key: string, at: number) => void;
}) {
	const title = citationTitle({ ...result, n: 0 });
	const time = formatClock(result.at);
	return (
		<li className={cn(cardClassName, ROW_ACTIVE)}>
			<button
				type="button"
				data-id={`search-result-${result.key}-${Math.floor(result.at)}`}
				onClick={() => onOpen(result.key, result.at)}
				className="grid min-h-11 w-full cursor-pointer gap-1.5 rounded-xl bg-transparent px-4 py-3 text-left text-fg focus-visible:shadow-focus-primary focus-visible:outline-none"
			>
				<span className="flex flex-wrap items-baseline gap-x-2 gap-y-1 text-muted text-sm">
					<span className="min-w-0 break-words font-semibold text-fg">
						{title}
					</span>
					{result.createdAt ? (
						<span>{formatDateTime(result.createdAt)}</span>
					) : null}
					<span className="font-mono text-primary tabular-nums">· {time}</span>
				</span>
				<span className="grid gap-0.5 text-sm leading-[1.6]">
					{result.before ? (
						<span className="line-clamp-1 break-words text-muted">
							{result.before}
						</span>
					) : null}
					<span className="break-words">
						{highlightParts(result.text, query).map((part, index) =>
							part.match ? (
								<mark
									// Parts of one sentence never move.
									// biome-ignore lint/suspicious/noArrayIndexKey: fixed-order parts
									key={index}
									className="rounded-sm bg-primary/15 px-0.5 font-semibold text-fg"
								>
									{part.text}
								</mark>
							) : (
								// biome-ignore lint/suspicious/noArrayIndexKey: fixed-order parts
								<span key={index}>{part.text}</span>
							),
						)}
					</span>
					{result.after ? (
						<span className="line-clamp-1 break-words text-muted">
							{result.after}
						</span>
					) : null}
				</span>
			</button>
		</li>
	);
}
