import type { ReactNode } from "react";
import { Link } from "react-router";
import { cardClassName } from "../components/ui/card";
import { useI18n } from "../i18n/i18n-context";
import type { MessageKey, ParamsOf } from "../i18n/message-types";
import { type TagRenderers, renderRich } from "../i18n/rich-text";
import { PATHS } from "../layout/app-paths";
import { LegalLinks, TEXT_LINK } from "../layout/legal-links";
import { PageHeader } from "../layout/page-header";
import { cn } from "../lib/cn";
import { IDP_ACCOUNT_URL } from "../lib/idp";
import {
	LEGAL_EFFECTIVE_DATE,
	PROCESSOR_POLICY_URLS,
	SUPPORT_EMAIL,
} from "../lib/legal";

export type LegalDoc = "privacy" | "terms";

/** Keys of texts without placeholders: `t(key)` takes no params. */
type PlainKey = {
	[K in MessageKey]: ParamsOf<K> extends undefined ? K : never;
}[MessageKey];

type Section = readonly [id: string, title: PlainKey, body: PlainKey];

const PRIVACY_SECTIONS: readonly Section[] = [
	["du-lieu", "legal.privacy.collect.title", "legal.privacy.collect.body"],
	["muc-dich", "legal.privacy.purposes.title", "legal.privacy.purposes.body"],
	[
		"ben-xu-ly",
		"legal.privacy.processors.title",
		"legal.privacy.processors.body",
	],
	["luu-giu", "legal.privacy.retention.title", "legal.privacy.retention.body"],
	["quyen", "legal.privacy.rights.title", "legal.privacy.rights.body"],
	["bao-mat", "legal.privacy.security.title", "legal.privacy.security.body"],
	["tre-em", "legal.privacy.children.title", "legal.privacy.children.body"],
	["thay-doi", "legal.privacy.changes.title", "legal.privacy.changes.body"],
];

const TERMS_SECTIONS: readonly Section[] = [
	["tai-khoan", "legal.terms.account.title", "legal.terms.account.body"],
	["goi", "legal.terms.plans.title", "legal.terms.plans.body"],
	["luu-tru", "legal.terms.storage.title", "legal.terms.storage.body"],
	["noi-dung", "legal.terms.content.title", "legal.terms.content.body"],
	["su-dung", "legal.terms.use.title", "legal.terms.use.body"],
	["ket-qua-may", "legal.terms.ai.title", "legal.terms.ai.body"],
	["key-rieng", "legal.terms.keys.title", "legal.terms.keys.body"],
	["dich-vu", "legal.terms.service.title", "legal.terms.service.body"],
	["cham-dut", "legal.terms.end.title", "legal.terms.end.body"],
	["luat", "legal.terms.law.title", "legal.terms.law.body"],
	["thay-doi", "legal.terms.changes.title", "legal.terms.changes.body"],
];

const DOCS = {
	privacy: {
		title: "legal.privacy.title",
		intro: "legal.privacy.intro",
		english: "legal.privacy.english",
		sections: PRIVACY_SECTIONS,
	},
	terms: {
		title: "legal.terms.title",
		intro: "legal.terms.intro",
		english: "legal.terms.english",
		sections: TERMS_SECTIONS,
	},
} as const satisfies Record<
	LegalDoc,
	{
		title: PlainKey;
		intro: PlainKey;
		english: PlainKey;
		sections: readonly Section[];
	}
>;

function externalLink(href: string) {
	// In the iOS app Capacitor opens target=_blank links in the system browser.
	return (chunk: string) => (
		<a
			href={href}
			target="_blank"
			rel="noopener noreferrer"
			className={TEXT_LINK}
		>
			{chunk}
		</a>
	);
}

/** Links the legal texts open with their tags. */
const LINK_TAGS: TagRenderers = {
	...Object.fromEntries(
		Object.entries(PROCESSOR_POLICY_URLS).map(([tag, url]) => [
			tag,
			externalLink(url),
		]),
	),
	account: externalLink(IDP_ACCOUNT_URL),
	privacy: (chunk) => (
		<Link to={PATHS.privacy} className={TEXT_LINK}>
			{chunk}
		</Link>
	),
	contact: (chunk) =>
		SUPPORT_EMAIL ? (
			<a href={`mailto:${SUPPORT_EMAIL}`} className={TEXT_LINK}>
				{chunk}
			</a>
		) : (
			chunk
		),
};

/**
 * A section body: paragraphs split by a blank line; a paragraph whose lines
 * all start with "- " is a list.
 */
export function renderLegalBody(text: string): ReactNode {
	return text.split("\n\n").map((block) => {
		const lines = block.split("\n");
		if (lines.every((line) => line.startsWith("- "))) {
			return (
				<ul key={block} className="m-0 grid list-disc gap-2 pl-5">
					{lines.map((line) => (
						<li key={line}>{renderRich(line.slice(2), LINK_TAGS)}</li>
					))}
				</ul>
			);
		}
		return (
			<p key={block} className="m-0">
				{renderRich(block, LINK_TAGS)}
			</p>
		);
	});
}

/**
 * Privacy policy or terms of use, public (no sign-in): Vietnamese, then a
 * short English summary. Signed in, it renders inside the app shell;
 * otherwise inside `PublicLayout`.
 */
export function LegalPage({ doc }: { doc: LegalDoc }) {
	const { t, formatDate } = useI18n();
	const page = DOCS[doc];
	const contact = SUPPORT_EMAIL ?? t("legal.contactFallback");
	return (
		<article data-id={`legal-${doc}`} className="grid gap-4">
			<PageHeader
				title={t(page.title)}
				description={t("legal.effective", {
					date: formatDate(LEGAL_EFFECTIVE_DATE),
				})}
			/>
			<div
				className={cn(
					cardClassName,
					"grid gap-6 px-5 py-6 text-[0.9375rem] text-fg leading-relaxed sm:px-8",
				)}
			>
				<p className="m-0">{renderRich(t(page.intro), LINK_TAGS)}</p>
				{page.sections.map(([id, title, body]) => (
					<section
						key={id}
						id={id}
						aria-labelledby={`${id}-title`}
						className="grid gap-2"
					>
						<h2
							id={`${id}-title`}
							className="m-0 font-bold font-mono text-base"
						>
							{t(title)}
						</h2>
						{renderLegalBody(t(body))}
					</section>
				))}
				<section
					id="contact"
					aria-labelledby="contact-title"
					className="grid gap-2"
				>
					<h2 id="contact-title" className="m-0 font-bold font-mono text-base">
						{t("legal.contact.title")}
					</h2>
					<p className="m-0">
						{renderRich(t("legal.contact.body", { contact }), LINK_TAGS)}
					</p>
				</section>
				<section
					id="english"
					lang="en"
					aria-labelledby="english-title"
					className="grid gap-2 border-border-subtle border-t pt-6"
				>
					<h2 id="english-title" className="m-0 font-bold font-mono text-base">
						{t("legal.englishTitle")}
					</h2>
					<p className="m-0 text-muted">{t(page.english)}</p>
				</section>
			</div>
			<LegalLinks />
		</article>
	);
}
