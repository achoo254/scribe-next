import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render } from "@testing-library/react";
import type { ReactElement } from "react";
import { ToastProvider } from "../components/ui/toast";

/** Renders with a fresh query client (no retries) and the toast provider. */
export function renderWithProviders(ui: ReactElement) {
	const queryClient = new QueryClient({
		defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
	});
	return {
		queryClient,
		...render(
			<QueryClientProvider client={queryClient}>
				<ToastProvider>{ui}</ToastProvider>
			</QueryClientProvider>,
		),
	};
}
