import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { RouterProvider } from "react-router";
import { AuthProvider } from "./auth/auth-context";
import { ToastProvider } from "./components/ui/toast";
import { TooltipProvider } from "./components/ui/tooltip";
import { I18nProvider } from "./i18n/i18n-provider";
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

/** Providers above the router: none of them needs a route. */
export function App({ queryClient, router }: AppProps) {
	return (
		<I18nProvider>
			<QueryClientProvider client={queryClient}>
				<TooltipProvider>
					<ToastProvider>
						<AuthProvider>
							<RouterProvider router={router} />
						</AuthProvider>
					</ToastProvider>
				</TooltipProvider>
			</QueryClientProvider>
		</I18nProvider>
	);
}
