import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App, createQueryClient } from "./app";
import { watchThemeColor } from "./lib/theme";
import { createAppRouter } from "./routes";
import "./styles/global.css";

const container = document.getElementById("app");
if (!container) throw new Error("index.html has no #app element");

watchThemeColor();

createRoot(container).render(
	<StrictMode>
		<App queryClient={createQueryClient()} router={createAppRouter()} />
	</StrictMode>,
);
