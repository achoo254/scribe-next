import {
	IconArrowRight,
	IconFileText,
	IconMicrophone,
	IconNotes,
	IconScissors,
} from "@tabler/icons-react";
import type { ReactNode } from "react";
import { Link } from "react-router";
import art360 from "../../../brand/art-scribe-360.webp";
import art720 from "../../../brand/art-scribe-720.webp";
import { Badge } from "../../components/ui/badge";
import { buttonClassName } from "../../components/ui/button";
import { cardClassName } from "../../components/ui/card";
import { useT } from "../../i18n/i18n-context";
import { MAIN_ID, SkipLink } from "../../layout/skip-link";
import { cn } from "../../lib/cn";
import { useIdpName } from "../../lib/idp-name";
import { LandingFooter } from "./landing-footer";
import { LANDING_ANCHORS, LandingHeader } from "./landing-header";

const START_CLASS = cn(
	buttonClassName({ variant: "primary", size: "lg" }),
	"no-underline max-sm:w-full",
);

const EYEBROW =
	"m-0 font-mono font-semibold text-primary text-xs uppercase tracking-[0.12em]";

const H2 =
	"m-0 font-bold text-[clamp(1.625rem,4vw,2.25rem)] leading-tight tracking-[-0.02em]";

/** Feature cards, in order (keys written out: the dictionary test finds them). */
const FEATURES = [
	{
		id: "record",
		Icon: IconMicrophone,
		title: "landing.features.record.title",
		body: "landing.features.record.body",
	},
	{
		id: "split",
		Icon: IconScissors,
		title: "landing.features.split.title",
		body: "landing.features.split.body",
	},
	{
		id: "transcribe",
		Icon: IconFileText,
		title: "landing.features.transcribe.title",
		body: "landing.features.transcribe.body",
	},
	{
		id: "after",
		Icon: IconNotes,
		title: "landing.features.after.title",
		body: "landing.features.after.body",
	},
] as const;

/** "Bắt đầu miễn phí": the hero's and the closing band's call to action. */
function StartLink({ "data-id": dataId }: { "data-id": string }) {
	const t = useT();
	return (
		<Link to="/login" data-id={dataId} className={START_CLASS}>
			{t("landing.hero.start")}
			<IconArrowRight size={20} aria-hidden="true" />
		</Link>
	);
}

/** One block below the hero: eyebrow, h2 and optional lead, then content. */
function LandingSection({
	id,
	eyebrow,
	title,
	lead,
	children,
}: {
	id: string;
	eyebrow: string;
	title: string;
	lead?: string;
	children: ReactNode;
}) {
	return (
		<section
			id={id}
			data-id={`landing-${id}`}
			aria-labelledby={`${id}-title`}
			className="grid gap-8 py-12 md:py-20"
		>
			<div className="grid gap-3">
				<p className={EYEBROW}>{eyebrow}</p>
				<h2 id={`${id}-title`} className={H2}>
					{title}
				</h2>
				{lead ? (
					<p className="m-0 max-w-160 text-[1.0625rem] text-muted">{lead}</p>
				) : null}
			</div>
			{children}
		</section>
	);
}

function Hero() {
	const t = useT();
	const idpName = useIdpName();
	return (
		<section
			data-id="landing-hero"
			aria-labelledby="landing-hero-title"
			className="grid items-center gap-10 pt-10 pb-8 md:pt-16 lg:grid-cols-[1.1fr_0.9fr] lg:gap-12 lg:pt-20 lg:pb-12"
		>
			<div className="grid min-w-0 justify-items-start gap-6">
				<Badge tone="primary">{t("landing.hero.eyebrow")}</Badge>
				<h1
					id="landing-hero-title"
					className="m-0 text-balance font-bold text-[clamp(2.25rem,6vw,3.5rem)] leading-[1.08] tracking-[-0.03em]"
				>
					{t("landing.hero.title")}
				</h1>
				<p className="m-0 max-w-140 text-lg text-muted">
					{t("landing.hero.lede")}
				</p>
				<div className="flex w-full flex-wrap gap-3">
					<StartLink data-id="landing-hero-start" />
					<a
						href={`#${LANDING_ANCHORS.features}`}
						data-id="landing-hero-features"
						className={cn(
							buttonClassName({ variant: "secondary", size: "lg" }),
							"no-underline max-sm:w-full",
						)}
					>
						{t("landing.hero.features")}
					</a>
				</div>
				<p className="m-0 text-muted text-sm">
					{t("landing.hero.note", { idpName })}
				</p>
			</div>
			{/* Decorative: the copy beside it says everything the picture shows. */}
			<div className="relative grid place-items-center">
				<div
					aria-hidden="true"
					className="absolute inset-[12%] rounded-full bg-primary/18 blur-3xl"
				/>
				<img
					src={art720}
					srcSet={`${art360} 360w, ${art720} 720w`}
					sizes="(min-width: 64rem) 28rem, 18rem"
					width={720}
					height={720}
					alt=""
					data-id="landing-hero-art"
					className="relative h-auto w-72 max-w-full lg:w-md"
				/>
			</div>
		</section>
	);
}

function Features() {
	const t = useT();
	return (
		<LandingSection
			id={LANDING_ANCHORS.features}
			eyebrow={t("landing.features.eyebrow")}
			title={t("landing.features.title")}
			lead={t("landing.features.lead")}
		>
			<ul className="m-0 grid list-none gap-4 p-0 sm:grid-cols-2">
				{FEATURES.map(({ id, Icon, title, body }) => (
					<li
						key={id}
						data-id={`landing-feature-${id}`}
						className={cn(cardClassName, "grid content-start gap-3 p-5 sm:p-6")}
					>
						<span className="tint-soft grid size-11 place-items-center rounded-md border text-primary [--tone:var(--c-primary)]">
							<Icon size={22} aria-hidden="true" />
						</span>
						<h3 className="m-0 font-bold text-lg">{t(title)}</h3>
						<p className="m-0 text-muted">{t(body)}</p>
					</li>
				))}
			</ul>
		</LandingSection>
	);
}

function HowItWorks() {
	const t = useT();
	const idpName = useIdpName();
	const steps = [
		{
			id: "sign-in",
			title: t("landing.how.steps.signIn.title"),
			body: t("landing.how.steps.signIn.body", { idpName }),
		},
		{
			id: "capture",
			title: t("landing.how.steps.capture.title"),
			body: t("landing.how.steps.capture.body"),
		},
		{
			id: "read",
			title: t("landing.how.steps.read.title"),
			body: t("landing.how.steps.read.body"),
		},
	];
	return (
		<LandingSection
			id={LANDING_ANCHORS.how}
			eyebrow={t("landing.how.eyebrow")}
			title={t("landing.how.title")}
		>
			<ol className="m-0 grid list-none gap-6 p-0 md:grid-cols-3">
				{steps.map(({ id, title, body }, index) => (
					<li
						key={id}
						data-id={`landing-step-${id}`}
						className="grid content-start gap-2 border-border border-t pt-5"
					>
						<span
							aria-hidden="true"
							className="font-bold font-mono text-primary text-sm"
						>
							{String(index + 1).padStart(2, "0")}
						</span>
						<h3 className="m-0 font-bold text-lg">{title}</h3>
						<p className="m-0 text-muted">{body}</p>
					</li>
				))}
			</ol>
		</LandingSection>
	);
}

function ClosingCta() {
	const t = useT();
	return (
		<section
			data-id="landing-cta"
			aria-labelledby="landing-cta-title"
			className={cn(
				cardClassName,
				"mt-4 mb-16 flex flex-wrap items-center justify-between gap-6 p-6 sm:p-8 md:mb-24",
			)}
		>
			<div className="grid max-w-150 gap-2">
				<h2
					id="landing-cta-title"
					className="m-0 font-bold text-[clamp(1.375rem,3vw,1.75rem)] leading-tight tracking-[-0.02em]"
				>
					{t("landing.cta.title")}
				</h2>
				<p className="m-0 text-muted">{t("landing.cta.body")}</p>
			</div>
			<StartLink data-id="landing-cta-start" />
		</section>
	);
}

/**
 * Public landing at `/` for visitors: hero, features, the three steps and
 * a closing call to action, between its own top bar and footer. Every
 * call to action goes to `/login`, which keeps the sign-in card. Signed-in
 * users never see it (`/` takes them to the app).
 */
export function LandingPage() {
	return (
		<div data-id="landing" className="flex min-h-dvh flex-col">
			<SkipLink />
			<LandingHeader />
			<main
				id={MAIN_ID}
				tabIndex={-1}
				className="mx-auto w-full max-w-page flex-1 px-gutter focus:outline-none"
			>
				<Hero />
				<Features />
				<HowItWorks />
				<ClosingCta />
			</main>
			<LandingFooter />
		</div>
	);
}
