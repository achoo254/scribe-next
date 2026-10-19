import { type ReactNode, Suspense, lazy } from "react";
import { Navigate, type RouteObject, createBrowserRouter } from "react-router";
import { useAuth } from "./auth/auth-context";
import { RequireAuth, RequireGuest } from "./auth/require-auth";
import { PATHS } from "./layout/app-paths";
import { AppShell } from "./layout/app-shell";
import { PageError } from "./layout/page-error";
import { PUBLIC_ROUTE, PublicLayout } from "./layout/public-layout";
import { SignedInState } from "./layout/signed-in-state";
import { LegalPage } from "./pages/legal-page";
import { LoginPage } from "./pages/login-page";
import { RecordPage } from "./pages/record/record-page";
import { SearchPage } from "./pages/search/search-page";
import { SplitPage } from "./pages/split/split-page";
import {
	TranscriptListPage,
	TranscriptReaderPage,
} from "./pages/transcript/transcript-page";

/** Development-only primitive gallery; never part of the production bundle. */
const UiGalleryPage = import.meta.env.DEV
	? lazy(() =>
			import("./pages/dev/ui-gallery-page").then((module) => ({
				default: module.UiGalleryPage,
			})),
		)
	: null;

/** The Transcript pages exist only for users who may transcribe. */
function RequireTranscribe({ children }: { children: ReactNode }) {
	const { user } = useAuth();
	if (!user?.canTranscribe) return <Navigate to={PATHS.record} replace />;
	return children;
}

/**
 * Every page has a URL. The signed-in pages share the shell and, above
 * it, `SignedInState` (recording, "Cắt file" upload and job), placed under
 * `RequireAuth` so signing out unmounts it. Pages are not lazy: a tab left
 * open across a deploy must not ask for a chunk that no longer exists.
 * The legal pages are public (`PUBLIC_ROUTE`): a visitor gets them in
 * `PublicLayout`, a signed-in user inside the shell, so opening them while
 * recording does not unmount the recording.
 */
export const appRoutes: RouteObject[] = [
	{
		path: "/",
		element: (
			<RequireAuth guest={<PublicLayout />}>
				<SignedInState>
					<AppShell />
				</SignedInState>
			</RequireAuth>
		),
		children: [
			{
				// Pathless layout route: a page that fails to render is replaced
				// here, below the shell, without unmounting `SignedInState`.
				errorElement: <PageError />,
				children: [
					{ index: true, element: <Navigate to={PATHS.record} replace /> },
					{ path: PATHS.record, element: <RecordPage /> },
					{ path: PATHS.split, element: <SplitPage /> },
					{
						path: PATHS.transcripts,
						element: (
							<RequireTranscribe>
								<TranscriptListPage />
							</RequireTranscribe>
						),
					},
					{
						path: PATHS.search,
						element: (
							<RequireTranscribe>
								<SearchPage />
							</RequireTranscribe>
						),
					},
					{
						path: `${PATHS.transcripts}/:key`,
						element: (
							<RequireTranscribe>
								<TranscriptReaderPage />
							</RequireTranscribe>
						),
					},
					{
						path: PATHS.privacy,
						element: <LegalPage doc="privacy" />,
						handle: PUBLIC_ROUTE,
					},
					{
						path: PATHS.terms,
						element: <LegalPage doc="terms" />,
						handle: PUBLIC_ROUTE,
					},
					// Unknown signed-in paths stay inside the layout: no provider remount.
					{ path: "*", element: <Navigate to={PATHS.record} replace /> },
				],
			},
		],
	},
	{
		path: "/login",
		element: (
			<RequireGuest>
				<LoginPage />
			</RequireGuest>
		),
	},
	...(UiGalleryPage
		? [
				{
					path: "/__ui",
					element: (
						<Suspense fallback={null}>
							<UiGalleryPage />
						</Suspense>
					),
				},
			]
		: []),
];

export type AppRouter = ReturnType<typeof createBrowserRouter>;

export function createAppRouter(): AppRouter {
	return createBrowserRouter(appRoutes);
}
