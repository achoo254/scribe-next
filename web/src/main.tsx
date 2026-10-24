import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App, createQueryClient } from "./app";
import { loadTenantInfo } from "./lib/tenant-info";
import { watchThemeColor } from "./lib/theme";
import { createAppRouter } from "./routes";
import "./styles/global.css";

const container = document.getElementById("app");
if (!container) throw new Error("index.html has no #app element");

watchThemeColor();
// The tenant of this host (Scribe on or not, its IdP) and the sign-in account's
// name for every text that names it (defaults until then).
void loadTenantInfo();

createRoot(container).render(
	<StrictMode>
		<App queryClient={createQueryClient()} router={createAppRouter()} />
	</StrictMode>,
);
