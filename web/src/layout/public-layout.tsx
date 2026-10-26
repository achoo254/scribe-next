import { Navigate, Outlet, useMatches } from "react-router";
import { isNativeShell } from "../lib/api";
import { cn } from "../lib/cn";
import { LandingPage } from "../pages/landing/landing-page";
import { PublicFooter, PublicHeader } from "./public-chrome";
import { MAIN_CLASS, MAIN_ID, SkipLink } from "./skip-link";

/** `handle` of a route a visitor may open without signing in. */
export const PUBLIC_ROUTE = { public: true } as const;

/** `handle` of the route a visitor sees as the landing page (`/`). */
export const LANDING_ROUTE = { landing: true } as const;

function hasHandleFlag(handle: unknown, flag: "public" | "landing"): boolean {
	return (
		typeof handle === "object" &&
		handle !== null &&
		Reflect.get(handle, flag) === true
	);
}

/**
 * What a visitor sees on the signed-in route tree: the landing page at `/`
 * (the iPhone and Android apps go straight to /login: an installed app has
 * nothing to advertise), the public pages (privacy, terms) between the
 * public top bar and footer; any other URL goes to /login.
 */
export function PublicLayout() {
	const matches = useMatches();
	if (matches.some((match) => hasHandleFlag(match.handle, "landing"))) {
		return isNativeShell() ? <Navigate to="/login" replace /> : <LandingPage />;
	}
	if (!matches.some((match) => hasHandleFlag(match.handle, "public"))) {
		return <Navigate to="/login" replace />;
	}
	return (
		<div
			data-id="public-layout"
			className="grid-backdrop flex min-h-dvh flex-col"
		>
			<SkipLink />
			<PublicHeader />
			<main id={MAIN_ID} tabIndex={-1} className={cn(MAIN_CLASS, "flex-1")}>
				<Outlet />
			</main>
			<PublicFooter />
		</div>
	);
}
