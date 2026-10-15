import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router";
import { App, createQueryClient } from "./app";
import { watchThemeColor } from "./lib/theme";
import "./styles/global.css";

const container = document.getElementById("app");
if (!container) throw new Error("index.html has no #app element");

watchThemeColor();

createRoot(container).render(
	<StrictMode>
		<BrowserRouter>
			<App queryClient={createQueryClient()} />
		</BrowserRouter>
	</StrictMode>,
);
