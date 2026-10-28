import { Navigate, Outlet, useMatches } from "react-router";
import { PublicI18nProvider } from "../i18n/public-i18n";
import { LandingPage } from "../pages/landing/landing-page";
import { ScribePublicPage } from "./scribe-public-page";

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

function PublicRoute() {
	const matches = useMatches();
	if (matches.some((match) => hasHandleFlag(match.handle, "landing"))) {
		return <LandingPage />;
	}
	if (!matches.some((match) => hasHandleFlag(match.handle, "public"))) {
		return <Navigate to="/login" replace />;
	}
	return (
		<ScribePublicPage>
			<Outlet />
		</ScribePublicPage>
	);
}

/**
 * What a visitor sees on the signed-in route tree, in the visitor's
 * language: the landing page at `/` (on the web and in the iPhone and
 * Android apps alike), the public pages (privacy, terms) in the shared
 * signed-out frame; any other URL goes to /login.
 */
export function PublicLayout() {
	return (
		<PublicI18nProvider>
			<PublicRoute />
		</PublicI18nProvider>
	);
}
