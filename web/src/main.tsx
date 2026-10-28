import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App, createQueryClient } from "./app";
import { loadIdpName } from "./lib/idp-name";
import { watchTheme } from "./lib/theme";
import { createAppRouter } from "./routes";
import "./styles/global.css";

const container = document.getElementById("app");
if (!container) throw new Error("index.html has no #app element");

watchTheme();
// The sign-in account's name for every text that names it (default until then).
void loadIdpName();

createRoot(container).render(
	<StrictMode>
		<App queryClient={createQueryClient()} router={createAppRouter()} />
	</StrictMode>,
);
