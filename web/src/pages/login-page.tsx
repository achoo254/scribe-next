import { useI18n } from "../i18n/i18n-context";
import { PublicI18nProvider } from "../i18n/public-i18n";
import { ScribePublicPage } from "../layout/scribe-public-page";
import { ScribeSignInCard } from "../layout/scribe-sign-in-card";
import { useIdpName } from "../lib/idp-name";

/**
 * Sign-in with the IdP account (hoandat-auth: Google, GitHub…): the shared
 * sign-in card alone in the shared signed-out frame (@achoo254/app-shell),
 * as on every hoandat.com app. The backend decides who may enter
 * (allowlist); the card says why a web sign-in came back (`?error=`) and
 * that an account was just deleted. In the visitor's language.
 */
export function LoginPage() {
	return (
		<PublicI18nProvider>
			<SignInPage />
		</PublicI18nProvider>
	);
}

function SignInPage() {
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
