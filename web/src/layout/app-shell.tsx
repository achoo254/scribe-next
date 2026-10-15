import {
	IconFileText,
	IconMicrophone,
	IconScissors,
} from "@tabler/icons-react";
import { useState } from "react";
import { QuotaBar } from "../components/quota-bar";
import { Badge } from "../components/ui/badge";
import {
	Tabs,
	TabsContent,
	TabsList,
	TabsTrigger,
} from "../components/ui/tabs";
import { RecordPage } from "../pages/record/record-page";
import { SplitPage } from "../pages/split/split-page";
import { AppTopBar } from "./app-top-bar";

type AppTab = "record" | "split";

function isAppTab(value: string): value is AppTab {
	return value === "record" || value === "split";
}

/**
 * The signed-in app: top bar, three tabs (Transcript is a placeholder, as
 * on desktop), quota. Both working tabs stay mounted while hidden, so a
 * recording or a split job keeps running when the user switches tabs.
 */
export function AppShell() {
	const [tab, setTab] = useState<AppTab>("record");

	return (
		<div
			data-id="app-shell"
			className="mx-auto grid w-full max-w-230 gap-4 px-gutter pt-4 pb-10 sm:pt-6"
		>
			<AppTopBar />
			<Tabs
				value={tab}
				onValueChange={(value) => {
					if (isAppTab(value)) setTab(value);
				}}
				className="grid gap-4"
			>
				<TabsList className="px-0" aria-label="Chức năng">
					<TabsTrigger value="record" data-id="tab-record" className="gap-2">
						<IconMicrophone size={18} aria-hidden="true" />
						Ghi âm
					</TabsTrigger>
					<TabsTrigger value="split" data-id="tab-split" className="gap-2">
						<IconScissors size={18} aria-hidden="true" />
						Cắt file
					</TabsTrigger>
					<TabsTrigger
						value="transcript"
						disabled
						data-id="tab-transcript"
						className="gap-2 disabled:cursor-not-allowed disabled:opacity-60"
					>
						<IconFileText size={18} aria-hidden="true" />
						Transcript
						<Badge tone="accent">Sắp có</Badge>
					</TabsTrigger>
				</TabsList>
				<QuotaBar />
				<TabsContent
					value="record"
					forceMount
					className="data-[state=inactive]:hidden"
				>
					<RecordPage />
				</TabsContent>
				<TabsContent
					value="split"
					forceMount
					className="data-[state=inactive]:hidden"
				>
					<SplitPage active={tab === "split"} />
				</TabsContent>
			</Tabs>
		</div>
	);
}
