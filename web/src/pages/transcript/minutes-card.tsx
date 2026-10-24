import { IconRefresh, IconSend, IconSparkles } from "@tabler/icons-react";
import { useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { ChatLinkDialog } from "../../components/chat-link-dialog";
import { Alert } from "../../components/ui/alert";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { ProgressBar } from "../../components/ui/progress-bar";
import { useToast } from "../../components/ui/toast";
import type { ChatLinkStatus } from "../../features/chat-link/chat-link-api";
import {
	chatLinkKeys,
	unlinkedStatus,
	useChatLinkQuery,
} from "../../features/chat-link/chat-link-queries";
import type { TranscriptMinutes } from "../../features/transcript-docs/transcript-docs-api";
import {
	useCreateSummary,
	useShareSummary,
} from "../../features/transcript-docs/transcript-docs-queries";
import { useT } from "../../i18n/i18n-context";
import type { Translate } from "../../i18n/translate";
import { ApiError, errorCode, errorMessage } from "../../lib/api";
import { formatDateTime, formatTimeAndDay } from "../../lib/format";
import { useTenantInfo } from "../../lib/tenant-info";
import { TimeJump } from "./time-jump";

export interface MinutesCardProps {
	transcriptKey: string;
	summary: TranscriptMinutes | null;
	/** At least one segment is transcribed: there is something to summarise. */
	canSummarize: boolean;
	onJump: (at: number) => void;
}

/** Why "Tóm tắt" was refused; a used-up allowance says when it opens again. */
export function summaryError(t: Translate, error: unknown): string {
	if (error instanceof ApiError && error.quota?.retryAt) {
		const time = formatTimeAndDay(error.quota.retryAt);
		if (error.code === "SUMMARY_DAILY_LIMIT") {
			return t("transcript.minutes.dailyLimit", {
				used: error.quota.used,
				limit: error.quota.limit,
				time,
			});
		}
		if (error.code === "SUMMARY_MONTHLY_LIMIT") {
			return t("transcript.minutes.monthlyLimit", {
				used: error.quota.used,
				limit: error.quota.limit,
				time,
			});
		}
		if (error.code === "CAP_REACHED") {
			return t("transcript.minutes.capReached", { time });
		}
	}
	return errorMessage(error);
}

/** The chat link is gone (never made, or the bot was blocked and the server unlinked). */
const CHAT_UNLINKED = new Set(["CHAT_NOT_LINKED", "CHAT_BLOCKED"]);

/**
 * "Biên bản" of the reader: nothing yet ("Tóm tắt"), being made, or the
 * minutes (summary, decisions, tasks; each item's time plays from there),
 * marked "Đã cũ" once the transcript changed. "Gửi sang chat" sends them
 * to the user's chat with the Scribe bot, or opens the linking steps first.
 */
export function MinutesCard({
	transcriptKey,
	summary,
	canSummarize,
	onJump,
}: MinutesCardProps) {
	const t = useT();
	const toast = useToast();
	const queryClient = useQueryClient();
	const create = useCreateSummary(transcriptKey);
	const share = useShareSummary(transcriptKey);
	// Sending to chat (bot of chat.hoandat.com) exists on the primary tenant only.
	const chatOffered = useTenantInfo().primary;
	const chatLink = useChatLinkQuery(summary !== null && chatOffered);
	const [linking, setLinking] = useState(false);
	const linked = chatLink.data?.linked === true;
	// The server takes no links: nothing to offer until one exists.
	const canSend = chatOffered && (linked || chatLink.data?.enabled !== false);

	function send(): void {
		if (!linked) {
			setLinking(true);
			return;
		}
		share.mutate(undefined, {
			onSuccess: () => toast.show({ title: t("transcript.minutes.sent") }),
			onError: (cause) => {
				const code = errorCode(cause);
				if (code !== null && CHAT_UNLINKED.has(code)) {
					queryClient.setQueryData<ChatLinkStatus>(
						chatLinkKeys.status,
						unlinkedStatus,
					);
					setLinking(true);
				}
				if (code !== "CHAT_NOT_LINKED") {
					toast.show({
						tone: "error",
						title: t("transcript.minutes.sendFailed"),
						description: errorMessage(cause),
					});
				}
			},
		});
	}

	const running = create.isPending;

	return (
		<Card data-id="minutes" className="grid gap-4 p-4 sm:p-6">
			<div className="flex flex-wrap items-center gap-x-3 gap-y-2">
				<h2 className="m-0 mr-auto flex items-center gap-2 font-mono font-semibold text-base">
					{t("transcript.minutes.title")}
					{summary?.stale ? (
						<Badge tone="warning" data-id="minutes-stale">
							{t("transcript.minutes.stale")}
						</Badge>
					) : null}
				</h2>
				{summary ? (
					<div className="flex flex-wrap gap-2">
						<Button
							size="sm"
							variant="ghost"
							data-id="minutes-recreate"
							loading={running}
							disabled={!canSummarize}
							leadingIcon={<IconRefresh size={16} aria-hidden="true" />}
							onClick={() => create.mutate()}
						>
							{t("transcript.minutes.recreate")}
						</Button>
						{canSend ? (
							<Button
								size="sm"
								data-id="minutes-send"
								loading={share.isPending || chatLink.isPending}
								leadingIcon={<IconSend size={16} aria-hidden="true" />}
								onClick={send}
							>
								{linked
									? t("transcript.minutes.send")
									: t("transcript.minutes.link")}
							</Button>
						) : null}
					</div>
				) : null}
			</div>

			{running ? (
				<div className="grid gap-2" aria-busy="true" data-id="minutes-running">
					<ProgressBar className="relative rounded-full" />
					<p className="m-0 text-muted text-sm" aria-live="polite">
						{t("transcript.minutes.running")}
					</p>
				</div>
			) : null}
			{create.isError ? (
				<Alert
					tone="danger"
					title={t("transcript.minutes.failed")}
					data-id="minutes-error"
				>
					{summaryError(t, create.error)}
				</Alert>
			) : null}

			{summary ? (
				<MinutesBody summary={summary} onJump={onJump} />
			) : running ? null : (
				<div className="grid justify-items-start gap-3">
					<p className="m-0 text-muted text-sm">
						{canSummarize
							? t("transcript.minutes.intro")
							: t("transcript.minutes.needsTranscript")}
					</p>
					<Button
						variant="primary"
						data-id="minutes-create"
						disabled={!canSummarize}
						leadingIcon={<IconSparkles size={18} aria-hidden="true" />}
						onClick={() => create.mutate()}
					>
						{t("transcript.minutes.create")}
					</Button>
				</div>
			)}

			{summary ? (
				<ChatLinkDialog open={linking} onOpenChange={setLinking} />
			) : null}
		</Card>
	);
}

function MinutesBody({
	summary,
	onJump,
}: {
	summary: TranscriptMinutes;
	onJump: (at: number) => void;
}) {
	const t = useT();
	return (
		<div className="grid gap-4">
			{summary.stale ? (
				<p className="m-0 text-sm text-warning-text">
					{t("transcript.minutes.staleBody")}
				</p>
			) : null}
			<p data-id="minutes-summary" className="m-0 whitespace-pre-line">
				{summary.summary}
			</p>

			<section className="grid gap-1" data-id="minutes-decisions">
				<h3 className="m-0 font-mono font-semibold text-[0.8125rem] text-muted">
					{t("transcript.minutes.decisions")}
				</h3>
				{summary.decisions.length > 0 ? (
					<ul className="m-0 grid list-none gap-0.5 p-0">
						{summary.decisions.map((item, index) => (
							<li
								// The minutes are replaced whole; their order never changes.
								// biome-ignore lint/suspicious/noArrayIndexKey: fixed-order list
								key={index}
								className="flex items-start gap-2"
							>
								<TimeJump
									at={item.at}
									onJump={onJump}
									data-id={`minutes-decision-${index}`}
								/>
								<span className="min-w-0 flex-1 pt-1.5 text-sm">
									{item.text}
								</span>
							</li>
						))}
					</ul>
				) : (
					<p className="m-0 text-muted text-sm">
						{t("transcript.minutes.noDecisions")}
					</p>
				)}
			</section>

			<section className="grid gap-1" data-id="minutes-actions">
				<h3 className="m-0 font-mono font-semibold text-[0.8125rem] text-muted">
					{t("transcript.minutes.actions")}
				</h3>
				{summary.actions.length > 0 ? (
					<ul className="m-0 grid list-none gap-0.5 p-0">
						{summary.actions.map((item, index) => (
							<li
								// biome-ignore lint/suspicious/noArrayIndexKey: fixed-order list
								key={index}
								className="flex items-start gap-2"
							>
								<TimeJump
									at={item.at}
									onJump={onJump}
									data-id={`minutes-action-${index}`}
								/>
								<span className="grid min-w-0 flex-1 gap-0.5 pt-1.5 text-sm">
									<span>{item.text}</span>
									{item.owner || item.due ? (
										<span className="text-muted text-xs">
											{[
												item.owner
													? t("transcript.minutes.owner", { owner: item.owner })
													: "",
												item.due
													? t("transcript.minutes.due", { due: item.due })
													: "",
											]
												.filter(Boolean)
												.join(" · ")}
										</span>
									) : null}
								</span>
							</li>
						))}
					</ul>
				) : (
					<p className="m-0 text-muted text-sm">
						{t("transcript.minutes.noActions")}
					</p>
				)}
			</section>

			<p className="m-0 text-muted text-xs">
				{t("transcript.minutes.createdAt", {
					date: formatDateTime(summary.createdAt),
				})}
			</p>
		</div>
	);
}
