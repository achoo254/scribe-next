import { useCallback, useState } from "react";
import { useAuth } from "../auth/auth-context";
import { QuotaBar } from "../components/quota-bar";
import { cn } from "../lib/cn";
import { RecordPage } from "../pages/record/record-page";
import { SplitPage } from "../pages/split/split-page";
import { TranscriptPage } from "../pages/transcript/transcript-page";
import { AppFooter } from "./app-footer";
import { AppHeader, type AppTab } from "./app-header";
import {
	SessionActivityProvider,
	useSessionActivityState,
} from "./session-activity";
import { MAIN_CLASS, MAIN_ID, SkipLink } from "./skip-link";
import { OpenTranscriptContext } from "./transcript-nav";

/**
 * The signed-in app: skip link, sticky top bar with the nav and the
 * account menu, quota, the pages, footer. Every page stays mounted while
 * hidden, so a recording, a split job or an open transcript keeps its
 * state when the user switches pages. The Transcript page exists only for
 * users who may transcribe.
 */
export function AppShell() {
	const { user } = useAuth();
	const canTranscribe = Boolean(user?.canTranscribe);
	const [selected, setTab] = useState<AppTab>("record");
	// The page disappears if the permission is withdrawn while it is open.
	const tab = selected === "transcript" && !canTranscribe ? "record" : selected;
	const [openKey, setOpenKey] = useState<string | null>(null);
	const openTranscript = useCallback((key: string) => {
		setOpenKey(key);
		setTab("transcript");
	}, []);
	const session = useSessionActivityState();

	return (
		<OpenTranscriptContext.Provider
			value={canTranscribe ? openTranscript : null}
		>
			<SessionActivityProvider value={session.publish}>
				<div data-id="app-shell" className="flex min-h-dvh flex-col">
					<SkipLink />
					{user ? (
						<AppHeader
							user={user}
							tab={tab}
							onTabChange={setTab}
							activity={session.activity}
							beforeSignOut={session.stopRecordings}
						/>
					) : null}
					<main id={MAIN_ID} tabIndex={-1} className={cn(MAIN_CLASS, "flex-1")}>
						<QuotaBar />
						<div data-id="page-record" hidden={tab !== "record"}>
							<RecordPage />
						</div>
						<div data-id="page-split" hidden={tab !== "split"}>
							<SplitPage active={tab === "split"} />
						</div>
						{canTranscribe ? (
							<div data-id="page-transcript" hidden={tab !== "transcript"}>
								<TranscriptPage
									active={tab === "transcript"}
									openKey={openKey}
									onOpenKeyChange={setOpenKey}
									canTranscribe={canTranscribe}
								/>
							</div>
						) : null}
					</main>
					<AppFooter />
				</div>
			</SessionActivityProvider>
		</OpenTranscriptContext.Provider>
	);
}
