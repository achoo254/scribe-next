import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { type ReactNode, useState } from "react";
import { ToastProvider } from "../components/ui/toast";
import { I18nProvider } from "../i18n/i18n-provider";

/**
 * The language of the shared choice (Vietnamese unless a test sets
 * `hd_lang`), query client (no retries) and toasts, for tests that bring
 * their own router (`renderWithProviders` adds a memory data router too).
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
		<I18nProvider>
			<QueryClientProvider client={queryClient}>
				<ToastProvider>{children}</ToastProvider>
			</QueryClientProvider>
		</I18nProvider>
	);
}
