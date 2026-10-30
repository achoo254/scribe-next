import { useI18n } from "../i18n/i18n-context";
import { ScribePublicPage } from "../layout/scribe-public-page";
import { ScribeSignInCard } from "../layout/scribe-sign-in-card";
import { useIdpName } from "../lib/idp-name";

/**
 * Sign-in with the IdP account (hoandat-auth: Google, GitHub, Apple…): the shared
 * sign-in card alone in the shared signed-out frame (@achoo254/app-shell),
 * as on every hoandat.com app. The backend decides who may enter
 * (allowlist); the card says why a web sign-in came back (`?error=`) and
 * that an account was just deleted.
 */
export function LoginPage() {
	const { t } = useI18n();
	const idpName = useIdpName();
	return (
		<ScribePublicPage layout="center">
			<ScribeSignInCard
				title={t("login.title")}
				description={t("login.subtitle", { idpName })}
			/>
		</ScribePublicPage>
	);
}
