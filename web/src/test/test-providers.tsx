import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { type ReactNode, useState } from "react";
import { ToastProvider } from "../components/ui/toast";

/**
 * Query client (no retries) and toasts, for tests that bring their own
 * router (`renderWithProviders` adds a memory data router too).
 */
export function TestProviders({ children }: { children: ReactNode }) {
	const [queryClient] = useState(
		() =>
			new QueryClient({
				defaultOptions: {
					queries: { retry: false },
					mutations: { retry: false },
				},
			}),
	);
	return (
		<QueryClientProvider client={queryClient}>
			<ToastProvider>{children}</ToastProvider>
		</QueryClientProvider>
	);
}
