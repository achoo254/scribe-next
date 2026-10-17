import { Outlet } from "react-router";
import { useAuth } from "../auth/auth-context";
import { RouteProgress } from "../components/ui/route-progress";
import { UsageCard } from "../components/usage-card";
import { cn } from "../lib/cn";
import { AppFooter } from "./app-footer";
import { AppHeader } from "./app-header";
import { MAIN_CLASS, MAIN_ID, SkipLink } from "./skip-link";

/**
 * The signed-in app: skip link, sticky top bar with the nav and the
 * account menu, plan and usage, the page of the URL, footer. A page unmounts when
 * the user leaves it; what must outlive it (recording, "Cắt file" upload
 * and job) lives in `SignedInState` around this shell.
 */
export function AppShell() {
	const { user } = useAuth();

	return (
		<div data-id="app-shell" className="flex min-h-dvh flex-col">
			<RouteProgress />
			<SkipLink />
			{user ? <AppHeader user={user} /> : null}
			<main id={MAIN_ID} tabIndex={-1} className={cn(MAIN_CLASS, "flex-1")}>
				<UsageCard />
				<Outlet />
			</main>
			<AppFooter />
		</div>
	);
}
