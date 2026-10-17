import { IconLanguage, IconRefresh } from "@tabler/icons-react";
import { Alert } from "../../components/ui/alert";
import { Button } from "../../components/ui/button";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuRadioGroup,
	DropdownMenuRadioItem,
	DropdownMenuTrigger,
} from "../../components/ui/dropdown-menu";
import { ProgressBar } from "../../components/ui/progress-bar";
import { SegmentedControl } from "../../components/ui/segmented-control";
import type {
	TranscriptTranslation,
	TranslationLang,
	TranslationProgress,
	TranslationState,
} from "../../features/transcript-docs/transcript-docs-api";
import { useStartTranslation } from "../../features/transcript-docs/transcript-docs-queries";
import {
	TRANSLATION_LANGS,
	TRANSLATION_LANG_LABEL,
} from "../../features/transcript-docs/translation-langs";
import { useT } from "../../i18n/i18n-context";
import type { Translate } from "../../i18n/translate";
import { ApiError, errorMessage } from "../../lib/api";
import { formatTimeAndDay } from "../../lib/format";

/** How the reader shows a translated transcript. */
export type TranslationView = "original" | "translation" | "both";

/** Why "Dịch" was refused; a used-up allowance says when it opens again. */
export function translateError(t: Translate, error: unknown): string {
	if (error instanceof ApiError && error.quota?.retryAt) {
		const time = formatTimeAndDay(error.quota.retryAt);
		if (error.code === "TRANSLATE_DAILY_LIMIT") {
			return t("transcript.translate.dailyLimit", {
				used: error.quota.used,
				limit: error.quota.limit,
				time,
			});
		}
		if (error.code === "TRANSLATE_MONTHLY_LIMIT") {
			return t("transcript.translate.monthlyLimit", {
				used: error.quota.used,
				limit: error.quota.limit,
				time,
			});
		}
		if (error.code === "CAP_REACHED") {
			return t("transcript.translate.capReached", { time });
		}
	}
	return errorMessage(error);
}

export interface TranslationControlsProps {
	transcriptKey: string;
	/** Translations stored for the transcript. */
	available: readonly TranslationState[];
	/** Language shown; null before any translation. */
	lang: TranslationLang | null;
	onLangChange: (lang: TranslationLang) => void;
	view: TranslationView;
	onViewChange: (view: TranslationView) => void;
	/** Sentences of `lang`, once loaded. */
	translation: TranscriptTranslation | undefined;
	/** Live progress of `lang` while a run goes. */
	progress: TranslationProgress | undefined;
	/** May start runs (needs the transcription right); reading needs none. */
	canTranslate: boolean;
}

/**
 * "Dịch" of the reader: pick the language (a new one starts translating,
 * one already stored is shown), progress while the server translates, the
 * view switch Gốc / Bản dịch / Song song, and "Dịch lại câu đã sửa" when
 * sentences changed since.
 */
export function TranslationControls({
	transcriptKey,
	available,
	lang,
	onLangChange,
	view,
	onViewChange,
	translation,
	progress,
	canTranslate,
}: TranslationControlsProps) {
	const t = useT();
	const start = useStartTranslation(transcriptKey);
	const languageName = (code: TranslationLang) =>
		t(TRANSLATION_LANG_LABEL[code]);
	const running = progress?.status === "running";
	const shown = translation && translation.status !== "none";
	const leftover = translation ? translation.outdated + translation.missing : 0;

	function run(target: TranslationLang): void {
		start.reset();
		start.mutate(target);
	}

	function choose(target: TranslationLang): void {
		onLangChange(target);
		if (view === "original") onViewChange("both");
		if (!available.some((a) => a.lang === target)) run(target);
	}

	return (
		<div data-id="translate" className="grid gap-3">
			<div className="flex flex-wrap items-center gap-2">
				{canTranslate ? (
					<DropdownMenu>
						<DropdownMenuTrigger asChild>
							<Button
								size="sm"
								data-id="translate-menu"
								loading={start.isPending}
								leadingIcon={<IconLanguage size={16} aria-hidden="true" />}
								className="pointer-coarse:min-h-11"
							>
								{lang && shown
									? t("transcript.translate.menuActive", {
											language: languageName(lang),
										})
									: t("transcript.translate.menu")}
							</Button>
						</DropdownMenuTrigger>
						<DropdownMenuContent align="start" data-id="translate-menu-content">
							<DropdownMenuRadioGroup
								value={lang ?? ""}
								onValueChange={(value) => {
									const target = TRANSLATION_LANGS.find((l) => l === value);
									if (target) choose(target);
								}}
							>
								{TRANSLATION_LANGS.map((code) => (
									<DropdownMenuRadioItem
										key={code}
										value={code}
										data-id={`translate-menu-${code}`}
									>
										{languageName(code)}
									</DropdownMenuRadioItem>
								))}
							</DropdownMenuRadioGroup>
						</DropdownMenuContent>
					</DropdownMenu>
				) : null}
				{shown ? (
					<SegmentedControl<TranslationView>
						legend={t("transcript.translate.view.legend")}
						data-id="translate-view"
						value={view}
						onChange={onViewChange}
						options={[
							{
								value: "original",
								label: t("transcript.translate.view.original"),
								"data-id": "translate-view-original",
							},
							{
								value: "translation",
								label: t("transcript.translate.view.translation"),
								"data-id": "translate-view-translation",
							},
							{
								value: "both",
								label: t("transcript.translate.view.both"),
								"data-id": "translate-view-both",
							},
						]}
					/>
				) : null}
			</div>

			{running && lang ? (
				<div
					className="grid gap-2"
					aria-busy="true"
					data-id="translate-running"
				>
					<ProgressBar className="relative rounded-full" />
					<p className="m-0 text-muted text-sm" aria-live="polite">
						{t("transcript.translate.running", {
							language: languageName(lang),
							done: progress.done,
							total: progress.total,
						})}
					</p>
				</div>
			) : start.isPending ? (
				<p className="m-0 text-muted text-sm" aria-live="polite">
					{t("transcript.translate.starting")}
				</p>
			) : null}

			{start.isError ? (
				<Alert
					tone="danger"
					title={t("transcript.translate.failed")}
					data-id="translate-error"
				>
					{translateError(t, start.error)}
				</Alert>
			) : progress?.status === "failed" && lang ? (
				<Alert
					tone="danger"
					title={t("transcript.translate.failed")}
					data-id="translate-failed"
				>
					{progress.error}
					{canTranslate ? (
						<Button
							size="sm"
							variant="ghost"
							className="ml-2"
							data-id="translate-resume"
							loading={start.isPending}
							onClick={() => run(lang)}
						>
							{t("transcript.translate.resume")}
						</Button>
					) : null}
				</Alert>
			) : !running && lang && translation && leftover > 0 ? (
				<Alert tone="warning" data-id="translate-outdated">
					{translation.outdated > 0
						? t("transcript.translate.outdated", {
								count: translation.outdated,
							})
						: t("transcript.translate.missing", { count: translation.missing })}
					{canTranslate ? (
						<Button
							size="sm"
							variant="ghost"
							className="ml-2"
							data-id="translate-refresh"
							loading={start.isPending}
							leadingIcon={<IconRefresh size={16} aria-hidden="true" />}
							onClick={() => run(lang)}
						>
							{translation.outdated > 0
								? t("transcript.translate.retranslate")
								: t("transcript.translate.fillMissing")}
						</Button>
					) : null}
				</Alert>
			) : null}
		</div>
	);
}
