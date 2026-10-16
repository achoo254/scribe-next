import { IconLogin2 } from "@tabler/icons-react";
import { useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { useAuth } from "../auth/auth-context";
import { loginErrorFromCode, loginErrorMessage } from "../auth/login-error";
import { BrandMark, Wordmark } from "../components/brand/wordmark";
import { ThemeToggle } from "../components/theme-toggle";
import { Alert } from "../components/ui/alert";
import { Button } from "../components/ui/button";
import { Card } from "../components/ui/card";

/**
 * Sign-in with the hoandat account (hoandat-auth: Google, GitHub…); the
 * backend decides who may enter (allowlist). A failed web sign-in comes
 * back here with `?error=<code>`.
 */
export function LoginPage() {
	const { login } = useAuth();
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
		<main
			data-id="login"
			className="grid min-h-dvh grid-rows-[auto_1fr] gap-4 px-gutter pt-[calc(var(--space-4)+env(safe-area-inset-top))] pb-[calc(var(--space-4)+env(safe-area-inset-bottom))]"
		>
			<div className="flex justify-end">
				<ThemeToggle data-id="login-theme" />
			</div>
			<div className="grid place-items-center pb-12">
				<Card className="grid w-full max-w-sm gap-8 px-6 py-10 sm:px-8">
					<div className="grid justify-items-center gap-2 text-center">
						<BrandMark size={48} />
						<h1 className="m-0">
							<Wordmark className="text-2xl" />
						</h1>
						<p className="m-0 text-muted text-sm">Ghi và cắt media</p>
					</div>
					<div className="grid gap-4">
						<Button
							variant="primary"
							size="lg"
							block
							loading={busy}
							data-id="login-submit"
							leadingIcon={<IconLogin2 size={20} aria-hidden="true" />}
							onClick={() => void signIn()}
						>
							Đăng nhập
						</Button>
						<p className="m-0 text-center text-muted text-sm">
							Dùng tài khoản hoandat (Google hoặc GitHub).
						</p>
						{error ? (
							<Alert tone="danger" data-id="login-error">
								{error}
							</Alert>
						) : null}
					</div>
				</Card>
			</div>
		</main>
	);
}
