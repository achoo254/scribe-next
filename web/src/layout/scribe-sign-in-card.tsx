import { SignInCard } from "@achoo254/app-shell";
import { useRef, useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { WEB_LOGIN_PATH } from "../auth/auth-api";
import { useAuth } from "../auth/auth-context";
import { loginErrorFromCode, loginErrorMessage } from "../auth/login-error";
import { Alert } from "../components/ui/alert";
import { useI18n } from "../i18n/i18n-context";
import { apiUrl } from "../lib/api";
import { useIdpName } from "../lib/idp-name";
import { SHELL_BRAND } from "./scribe-public-page";

/**
 * The shared sign-in card (@achoo254/app-shell) with Scribe's sign-in
 * behind its one call to action: the web follows the backend sign-in
 * (`/api/auth/login` → auth.hoandat.com, BFF), the iPhone and Android apps
 * open it in the system browser (`useAuth().login`). Under the card: why
 * the last web sign-in came back (`?error=<code>`), a failed app sign-in,
 * or that the account was just deleted.
 */
export function ScribeSignInCard({
	title,
	description,
	headingLevel,
}: {
	title: string;
	description: string;
	/** 2 beside the landing's hero, whose title is the h1. */
	headingLevel?: 1 | 2;
}) {
	const { locale, t } = useI18n();
	const { login, accountDeleted } = useAuth();
	const idpName = useIdpName();
	const navigate = useNavigate();
	const [params] = useSearchParams();
	const [failure, setFailure] = useState("");
	// The ref blocks a second tap before React re-renders; the state shows
	// the call to action as busy while the system browser is open.
	const busy = useRef(false);
	const [pending, setPending] = useState(false);
	// The code, not its text, is the state: the text follows the language.
	const error = failure || loginErrorFromCode(params.get("error"));

	async function signIn(): Promise<void> {
		// The native sign-in waits on the system browser: one at a time.
		if (busy.current) return;
		busy.current = true;
		setPending(true);
		setFailure("");
		try {
			await login();
			navigate("/", { replace: true });
		} catch (cause) {
			setFailure(loginErrorMessage(cause));
		} finally {
			busy.current = false;
			setPending(false);
		}
	}

	return (
		<SignInCard
			brand={SHELL_BRAND}
			locale={locale}
			signInHref={apiUrl(WEB_LOGIN_PATH)}
			onSignIn={() => void signIn()}
			pending={pending}
			title={title}
			description={description}
			headingLevel={headingLevel}
		>
			{accountDeleted ? (
				<Alert tone="success" data-id="login-account-deleted">
					{accountDeleted === "pending"
						? t("login.accountDeletedPending")
						: t("login.accountDeleted", { idpName })}
				</Alert>
			) : null}
			{error ? (
				<Alert tone="danger" data-id="login-error">
					{error}
				</Alert>
			) : null}
		</SignInCard>
	);
}
