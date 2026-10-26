import { IconLogin2 } from "@tabler/icons-react";
import { useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { useAuth } from "../auth/auth-context";
import { loginErrorFromCode, loginErrorMessage } from "../auth/login-error";
import { Alert } from "../components/ui/alert";
import { Button } from "../components/ui/button";
import { Card } from "../components/ui/card";
import { useT } from "../i18n/i18n-context";
import { PublicFooter, PublicHeader } from "../layout/public-chrome";
import { useIdpName } from "../lib/idp-name";

/**
 * Sign-in with the IdP account (hoandat-auth: Google, GitHub…), named by
 * its tenant (`useIdpName`); the
 * backend decides who may enter (allowlist). A failed web sign-in comes
 * back here with `?error=<code>`; after "Xoá tài khoản" the page says the
 * account is gone (`accountDeleted`). Laid out like the hoandat-auth sign-in
 * page: the 100px grid behind, the shared public top bar, one 420px
 * card 48px under it with a mono title, a subtitle, the button and a
 * muted note, and the public footer (language, hoandat.com, legal links).
 */
export function LoginPage() {
	const t = useT();
	const idpName = useIdpName();
	const { login, accountDeleted } = useAuth();
	const navigate = useNavigate();
	const [params] = useSearchParams();
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState(() =>
		loginErrorFromCode(params.get("error")),
	);

	async function signIn(): Promise<void> {
		setError("");
		setBusy(true);
		try {
			await login();
			navigate("/", { replace: true });
		} catch (cause) {
			setError(loginErrorMessage(cause));
		} finally {
			setBusy(false);
		}
	}

	return (
		<div
			data-id="login"
			className="grid-backdrop grid min-h-dvh grid-rows-[auto_1fr_auto]"
		>
			<PublicHeader />
			<main className="grid content-start justify-items-center px-gutter pt-12 pb-16">
				<Card className="grid w-full max-w-105 gap-4 px-6 py-8 sm:p-8">
					<div className="grid gap-1">
						<h1 className="m-0 font-bold font-mono text-[1.375rem] tracking-[-0.01em]">
							{t("login.title")}
						</h1>
						<p className="m-0 text-muted text-sm">
							{t("login.subtitle", { idpName })}
						</p>
					</div>
					{accountDeleted ? (
						<Alert tone="success" data-id="login-account-deleted">
							{accountDeleted === "pending"
								? t("login.accountDeletedPending")
								: t("login.accountDeleted", { idpName })}
						</Alert>
					) : null}
					<Button
						variant="primary"
						size="lg"
						block
						loading={busy}
						data-id="login-submit"
						className="py-3"
						leadingIcon={<IconLogin2 size={20} aria-hidden="true" />}
						onClick={() => void signIn()}
					>
						{/* The account name comes from the backend and may be long:
						    the label wraps instead of widening the card past a
						    phone screen. */}
						<span className="whitespace-normal text-center leading-snug">
							{t("login.submit", { idpName })}
						</span>
					</Button>
					{error ? (
						<Alert tone="danger" data-id="login-error">
							{error}
						</Alert>
					) : null}
					<p className="m-0 text-center text-muted text-sm">
						{t("login.note", { idpName })}
					</p>
				</Card>
			</main>
			<PublicFooter />
		</div>
	);
}
