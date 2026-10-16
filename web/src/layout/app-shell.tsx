import {
	IconFileText,
	IconMicrophone,
	IconScissors,
} from "@tabler/icons-react";
import { useCallback, useState } from "react";
import { useAuth } from "../auth/auth-context";
import { QuotaBar } from "../components/quota-bar";
import {
	Tabs,
	TabsContent,
	TabsList,
	TabsTrigger,
} from "../components/ui/tabs";
import { RecordPage } from "../pages/record/record-page";
import { SplitPage } from "../pages/split/split-page";
import { TranscriptPage } from "../pages/transcript/transcript-page";
import { AppTopBar } from "./app-top-bar";
import { OpenTranscriptContext } from "./transcript-nav";

type AppTab = "record" | "split" | "transcript";

function isAppTab(value: string): value is AppTab {
	return value === "record" || value === "split" || value === "transcript";
}

/**
 * The signed-in app: top bar, tabs, quota. Every tab stays mounted while
 * hidden, so a recording, a split job or an open transcript keeps its state
 * when the user switches tabs. The Transcript tab exists only for users who
 * may transcribe.
 */
export function AppShell() {
	const { user } = useAuth();
	const canTranscribe = Boolean(user?.canTranscribe);
	const [selected, setTab] = useState<AppTab>("record");
	// The tab disappears if the permission is withdrawn while it is open.
	const tab = selected === "transcript" && !canTranscribe ? "record" : selected;
	const [openKey, setOpenKey] = useState<string | null>(null);
	const openTranscript = useCallback((key: string) => {
		setOpenKey(key);
		setTab("transcript");
	}, []);

	return (
		<OpenTranscriptContext.Provider
			value={canTranscribe ? openTranscript : null}
		>
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
						{canTranscribe ? (
							<TabsTrigger
								value="transcript"
								data-id="tab-transcript"
								className="gap-2"
							>
								<IconFileText size={18} aria-hidden="true" />
								Transcript
							</TabsTrigger>
						) : null}
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
					{canTranscribe ? (
						<TabsContent
							value="transcript"
							forceMount
							className="data-[state=inactive]:hidden"
						>
							<TranscriptPage
								active={tab === "transcript"}
								openKey={openKey}
								onOpenKeyChange={setOpenKey}
								canTranscribe={canTranscribe}
							/>
						</TabsContent>
					) : null}
				</Tabs>
			</div>
		</OpenTranscriptContext.Provider>
	);
}
