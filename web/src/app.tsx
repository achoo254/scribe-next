import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { RouterProvider } from "react-router";
import { AuthProvider } from "./auth/auth-context";
import { SessionCheck } from "./auth/require-auth";
import { ToastProvider } from "./components/ui/toast";
import { TooltipProvider } from "./components/ui/tooltip";
import { I18nProvider } from "./i18n/i18n-provider";
import { isNativeShell } from "./lib/api";
import { useTenantInfo, useTenantSettled } from "./lib/tenant-info";
import {
	TenantDisabledPage,
	TenantUnavailablePage,
} from "./pages/tenant-disabled-page";
import type { AppRouter } from "./routes";

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

export interface AppProps {
	queryClient: QueryClient;
	/** `createAppRouter()` in the browser, a memory router in tests. */
	router: AppRouter;
}

/**
 * The app once the tenant of this host is known (`GET /api/idp`, read once
 * in main.tsx): a domain whose owner has not turned Scribe on gets the "not
 * enabled" page and no signed-in API is called (the session check waits
 * for the tenant). The native shell talks to the primary tenant only and
 * does not wait.
 */
export function TenantGate({ children }: { children: ReactNode }) {
	const settled = useTenantSettled();
	const tenant = useTenantInfo();
	if (isNativeShell()) return children;
	if (!settled) return <SessionCheck />;
	if (tenant.scribe === "disabled")
		return <TenantDisabledPage tenant={tenant} />;
	if (tenant.scribe === "unavailable") {
		return <TenantUnavailablePage tenant={tenant} />;
	}
	return children;
}

/** Providers above the router: none of them needs a route. */
export function App({ queryClient, router }: AppProps) {
	return (
		<I18nProvider>
			<QueryClientProvider client={queryClient}>
				<TooltipProvider>
					<ToastProvider>
						<TenantGate>
							<AuthProvider>
								<RouterProvider router={router} />
							</AuthProvider>
						</TenantGate>
					</ToastProvider>
				</TooltipProvider>
			</QueryClientProvider>
		</I18nProvider>
	);
}
