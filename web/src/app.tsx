import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Suspense, lazy } from "react";
import { Navigate, Route, Routes } from "react-router";
import { AuthProvider } from "./auth/auth-context";
import { RequireAuth, RequireGuest } from "./auth/require-auth";
import { ToastProvider } from "./components/ui/toast";
import { TooltipProvider } from "./components/ui/tooltip";
import { AppShell } from "./layout/app-shell";
import { LoginPage } from "./pages/login-page";

/** Development-only primitive gallery; never part of the staging bundle. */
const UiGalleryPage = import.meta.env.DEV
	? lazy(() =>
			import("./pages/dev/ui-gallery-page").then((module) => ({
				default: module.UiGalleryPage,
			})),
		)
	: null;

export function createQueryClient(): QueryClient {
	return new QueryClient({
		defaultOptions: {
			queries: {
				// Lists change only through this app: refresh on demand, not on focus.
				refetchOnWindowFocus: false,
				retry: 1,
			},
		},
	});
}

/** Routes: "/" needs a session, "/login" a visitor, anything else goes home. */
export function AppRoutes() {
	return (
		<Routes>
			<Route
				path="/"
				element={
					<RequireAuth>
						<AppShell />
					</RequireAuth>
				}
			/>
			<Route
				path="/login"
				element={
					<RequireGuest>
						<LoginPage />
					</RequireGuest>
				}
			/>
			{UiGalleryPage ? (
				<Route
					path="/__ui"
					element={
						<Suspense fallback={null}>
							<UiGalleryPage />
						</Suspense>
					}
				/>
			) : null}
			<Route path="*" element={<Navigate to="/" replace />} />
		</Routes>
	);
}

export function App({ queryClient }: { queryClient: QueryClient }) {
	return (
		<QueryClientProvider client={queryClient}>
			<TooltipProvider>
				<ToastProvider>
					<AuthProvider>
						<AppRoutes />
					</AuthProvider>
				</ToastProvider>
			</TooltipProvider>
		</QueryClientProvider>
	);
}
