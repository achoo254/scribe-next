import { IconMessageQuestion, IconSend } from "@tabler/icons-react";
import { type FormEvent, useState } from "react";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { Input } from "../../components/ui/input";
import { ProgressBar } from "../../components/ui/progress-bar";
import {
	type AskCitation,
	type AskReply,
	MAX_QUESTION_CHARS,
	answerParts,
} from "../../features/search/search-api";
import { useAsk } from "../../features/search/search-queries";
import { transcriptTitle } from "../../features/transcript-docs/reader-model";
import { useT } from "../../i18n/i18n-context";
import type { Translate } from "../../i18n/translate";
import { ApiError, errorMessage } from "../../lib/api";
import { formatClock, formatTimeAndDay } from "../../lib/format";

export interface AskCardProps {
	/** Ask about this transcript only; every transcript when absent. */
	transcriptKey?: string;
	/** A source was tapped: show and play the transcript from `citation.at`. */
	onCite: (citation: AskCitation) => void;
}

interface Entry {
	id: number;
	question: string;
	reply: AskReply | null;
	error: unknown;
}

/** Why a question was refused; a used-up allowance says when it opens again. */
export function askError(t: Translate, error: unknown): string {
	if (error instanceof ApiError && error.quota?.retryAt) {
		const time = formatTimeAndDay(error.quota.retryAt);
		const counts = { used: error.quota.used, limit: error.quota.limit, time };
		if (error.code === "ASK_DAILY_LIMIT") {
			return t("search.ask.dailyLimit", counts);
		}
		if (error.code === "ASK_MONTHLY_LIMIT") {
			return t("search.ask.monthlyLimit", counts);
		}
		if (error.code === "CAP_REACHED") {
			return t("search.ask.capReached", { time });
		}
	}
	return errorMessage(error);
}

/** Name of the transcript a citation comes from, as the list shows it. */
export function citationTitle(citation: AskCitation): string {
	return transcriptTitle({
		key: citation.key,
		title: citation.title,
		name: null,
		createdAt: citation.createdAt,
	});
}

/**
 * "Hỏi Scribe": a question in plain words, answered only from the user's
 * transcripts. Every `[n]` of the answer and every source below it plays
 * the transcript from that second. Questions stay on the page only (nothing
 * is saved); the newest is on top.
 */
export function AskCard({ transcriptKey, onCite }: AskCardProps) {
	const t = useT();
	const ask = useAsk(transcriptKey);
	const [draft, setDraft] = useState("");
	const [entries, setEntries] = useState<Entry[]>([]);
	const one = transcriptKey !== undefined;

	async function submit(event: FormEvent): Promise<void> {
		event.preventDefault();
		const question = draft.trim();
		if (!question || ask.isPending) return;
		const id = (entries[0]?.id ?? 0) + 1;
		setEntries((list) => [{ id, question, reply: null, error: null }, ...list]);
		setDraft("");
		const update = (patch: Partial<Entry>) =>
			setEntries((list) =>
				list.map((entry) => (entry.id === id ? { ...entry, ...patch } : entry)),
			);
		try {
			update({ reply: await ask.mutateAsync(question) });
		} catch (cause) {
			update({ error: cause });
		}
	}

	return (
		<Card data-id="ask" className="grid gap-4 p-4 sm:p-6">
			<div className="grid gap-1">
				<h2 className="m-0 flex items-center gap-2 font-mono font-semibold text-base">
					<IconMessageQuestion
						size={18}
						aria-hidden="true"
						className="text-primary"
					/>
					{t("search.ask.title")}
				</h2>
				<p className="m-0 text-muted text-sm">
					{one ? t("search.ask.introOne") : t("search.ask.introAll")}
				</p>
			</div>
			<form
				className="flex flex-wrap items-start gap-2"
				onSubmit={(event) => void submit(event)}
			>
				<Input
					data-id="ask-input"
					aria-label={t("search.ask.field")}
					placeholder={
						one
							? t("search.ask.placeholderOne")
							: t("search.ask.placeholderAll")
					}
					value={draft}
					maxLength={MAX_QUESTION_CHARS}
					className="min-w-0 flex-[1_1_16rem]"
					onChange={(event) => setDraft(event.target.value)}
				/>
				<Button
					type="submit"
					variant="primary"
					data-id="ask-submit"
					loading={ask.isPending}
					disabled={!draft.trim()}
					leadingIcon={<IconSend size={16} aria-hidden="true" />}
				>
					{t("search.ask.submit")}
				</Button>
			</form>
			{entries.length > 0 ? (
				<ol className="m-0 grid list-none gap-4 p-0" data-id="ask-entries">
					{entries.map((entry) => (
						<AskEntry key={entry.id} entry={entry} onCite={onCite} />
					))}
				</ol>
			) : null}
			{entries.length > 0 ? (
				<p className="m-0 text-muted text-xs">{t("search.ask.session")}</p>
			) : null}
		</Card>
	);
}

function AskEntry({
	entry,
	onCite,
}: {
	entry: Entry;
	onCite: (citation: AskCitation) => void;
}) {
	const t = useT();
	const dataId = `ask-entry-${entry.id}`;
	return (
		<li
			data-id={dataId}
			className="grid gap-2 border-border-subtle border-t pt-4 first:border-t-0 first:pt-0"
		>
			<p className="m-0 font-semibold text-sm">
				{t("search.ask.question", { question: entry.question })}
			</p>
			{entry.reply ? (
				<AskAnswer reply={entry.reply} dataId={dataId} onCite={onCite} />
			) : entry.error ? (
				<Alert
					tone="danger"
					title={t("search.ask.failed")}
					data-id={`${dataId}-error`}
				>
					{askError(t, entry.error)}
				</Alert>
			) : (
				<div
					className="grid gap-2"
					aria-busy="true"
					data-id={`${dataId}-running`}
				>
					<ProgressBar className="relative rounded-full" />
					<p className="m-0 text-muted text-sm" aria-live="polite">
						{t("search.ask.running")}
					</p>
				</div>
			)}
		</li>
	);
}

const CITE_CHIP =
	"mx-0.5 inline-flex min-h-6 cursor-pointer items-center rounded-sm border border-border bg-surface-subtle px-1.5 align-baseline font-mono text-primary text-xs tabular-nums hover:border-primary focus-visible:shadow-focus-primary focus-visible:outline-none pointer-coarse:min-h-8";

function AskAnswer({
	reply,
	dataId,
	onCite,
}: {
	reply: AskReply;
	dataId: string;
	onCite: (citation: AskCitation) => void;
}) {
	const t = useT();
	const label = (citation: AskCitation) =>
		t("search.ask.cite", {
			n: citation.n,
			title: citationTitle(citation),
			time: formatClock(citation.at),
		});
	return (
		<div className="grid gap-3">
			<p
				data-id={`${dataId}-answer`}
				className="m-0 whitespace-pre-line leading-[1.65]"
				aria-live="polite"
			>
				{answerParts(reply).map((part, index) =>
					part.kind === "text" ? (
						// The answer is replaced whole; its parts never move.
						// biome-ignore lint/suspicious/noArrayIndexKey: fixed-order parts
						<span key={index}>{part.text}</span>
					) : (
						<button
							// biome-ignore lint/suspicious/noArrayIndexKey: fixed-order parts
							key={index}
							type="button"
							data-id={`${dataId}-cite-${part.citation.n}`}
							aria-label={label(part.citation)}
							className={CITE_CHIP}
							onClick={() => onCite(part.citation)}
						>
							{part.citation.n}
						</button>
					),
				)}
			</p>
			{reply.citations.length > 0 ? (
				<section className="grid gap-1">
					<h3 className="m-0 font-mono font-semibold text-[0.8125rem] text-muted">
						{t("search.ask.sources")}
					</h3>
					<ul className="m-0 grid list-none gap-1 p-0">
						{reply.citations.map((citation) => (
							<li key={citation.n}>
								<button
									type="button"
									data-id={`${dataId}-source-${citation.n}`}
									onClick={() => onCite(citation)}
									className="flex min-h-11 w-full cursor-pointer items-start gap-2 rounded-md border-0 bg-transparent px-2 py-1.5 text-left text-fg hover:bg-surface-subtle focus-visible:shadow-focus-primary focus-visible:outline-none"
								>
									<span className="flex-none pt-0.5 font-mono text-primary text-xs tabular-nums">
										{citation.n}
									</span>
									<span className="grid min-w-0 flex-1 gap-0.5 text-sm">
										<span className="flex flex-wrap gap-x-2 text-muted text-xs">
											<span className="min-w-0 break-words">
												{citationTitle(citation)}
											</span>
											<span className="font-mono tabular-nums">
												{formatClock(citation.at)}
											</span>
										</span>
										<span className="line-clamp-2 break-words">
											{citation.text}
										</span>
									</span>
								</button>
							</li>
						))}
					</ul>
				</section>
			) : null}
		</div>
	);
}
