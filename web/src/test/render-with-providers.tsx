import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render } from "@testing-library/react";
import type { ReactElement } from "react";
import {
	type RouteObject,
	RouterProvider,
	createMemoryRouter,
} from "react-router";
import { ToastProvider } from "../components/ui/toast";

export interface RenderOptions {
	/** More routes next to `ui` (at "/"), e.g. a page to navigate to. */
	routes?: RouteObject[];
}

/**
 * Renders `ui` at "/" of a memory data router (pages may use `useBlocker`
 * and `useNavigate`) with a fresh query client (no retries) and the toast
 * provider.
 */
export function renderWithProviders(
	ui: ReactElement,
	{ routes = [] }: RenderOptions = {},
) {
	const queryClient = new QueryClient({
		defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
	});
	const router = createMemoryRouter([{ path: "/", element: ui }, ...routes]);
	return {
		queryClient,
		router,
		...render(
			<QueryClientProvider client={queryClient}>
				<ToastProvider>
					<RouterProvider router={router} />
				</ToastProvider>
			</QueryClientProvider>,
		),
	};
}
