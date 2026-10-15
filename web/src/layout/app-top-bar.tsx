import { IconLogout } from "@tabler/icons-react";
import { useState } from "react";
import { useNavigate } from "react-router";
import { useAuth } from "../auth/auth-context";
import { BrandMark, Wordmark } from "../components/brand/wordmark";
import { ThemeToggle } from "../components/theme-toggle";
import { Button } from "../components/ui/button";

/** Brand on the left; account, theme and sign-out on the right. */
export function AppTopBar() {
	const { user, logout } = useAuth();
	const navigate = useNavigate();
	const [leaving, setLeaving] = useState(false);

	async function signOut(): Promise<void> {
		setLeaving(true);
		await logout();
		navigate("/login", { replace: true });
	}

	return (
		<header
			data-id="topbar"
			className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2"
		>
			<span className="flex items-center gap-2">
				<BrandMark size={28} />
				<Wordmark />
			</span>
			<div className="flex min-w-0 flex-wrap items-center justify-end gap-2">
				{user ? (
					<span
						data-id="topbar-email"
						className="min-w-0 truncate text-muted text-sm"
						title={user.email}
					>
						{user.email}
					</span>
				) : null}
				<ThemeToggle data-id="topbar-theme" />
				<Button
					variant="ghost"
					size="sm"
					loading={leaving}
					data-id="topbar-logout"
					leadingIcon={<IconLogout size={16} aria-hidden="true" />}
					onClick={() => void signOut()}
				>
					Đăng xuất
				</Button>
			</div>
		</header>
	);
}
