import { FeatureList, Hero } from "@achoo254/app-shell";
import {
	IconFileText,
	IconMicrophone,
	IconNotes,
	IconScissors,
} from "@tabler/icons-react";
import { useI18n } from "../../i18n/i18n-context";
import { ScribePublicPage } from "../../layout/scribe-public-page";
import { ScribeSignInCard } from "../../layout/scribe-sign-in-card";
import { useIdpName } from "../../lib/idp-name";

/** In-page anchors of the landing sections. */
const LANDING_ANCHORS = { features: "features", how: "how" } as const;

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

/**
 * Public landing at `/` for visitors, on the web and in the iPhone and
 * Android apps: Scribe's hero beside the shared sign-in card (the page's
 * one call to action), its features and the three steps, in the shared
 * signed-out frame (@achoo254/app-shell). Signed-in users never see it
 * (`/` takes them to the app).
 */
export function LandingPage() {
	const { t } = useI18n();
	const idpName = useIdpName();
	const steps = [
		{
			id: "sign-in",
			title: t("landing.how.steps.signIn.title"),
			description: t("landing.how.steps.signIn.body", { idpName }),
		},
		{
			id: "capture",
			title: t("landing.how.steps.capture.title"),
			description: t("landing.how.steps.capture.body"),
		},
		{
			id: "read",
			title: t("landing.how.steps.read.title"),
			description: t("landing.how.steps.read.body"),
		},
	];
	return (
		<ScribePublicPage
			navLinks={[
				{
					href: `#${LANDING_ANCHORS.features}`,
					label: t("landing.nav.features"),
					dataId: "landing-nav-features",
					hideOnPhones: true,
				},
				{
					href: `#${LANDING_ANCHORS.how}`,
					label: t("landing.nav.how"),
					dataId: "landing-nav-how",
					hideOnPhones: true,
				},
			]}
		>
			<Hero
				eyebrow={t("landing.hero.eyebrow")}
				title={t("landing.hero.title")}
				lede={t("landing.hero.lede")}
				aside={
					<ScribeSignInCard
						headingLevel={2}
						title={t("landing.signIn.title")}
						description={t("landing.signIn.body")}
					/>
				}
			/>
			<FeatureList
				id={LANDING_ANCHORS.features}
				eyebrow={t("landing.features.eyebrow")}
				title={t("landing.features.title")}
				items={FEATURES.map(({ id, Icon, title, body }) => ({
					id,
					title: t(title),
					description: t(body),
					icon: <Icon size={20} aria-hidden="true" />,
				}))}
			/>
			<FeatureList
				id={LANDING_ANCHORS.how}
				eyebrow={t("landing.how.eyebrow")}
				title={t("landing.how.title")}
				items={steps.map((step, index) => ({
					...step,
					icon: String(index + 1).padStart(2, "0"),
				}))}
			/>
		</ScribePublicPage>
	);
}
