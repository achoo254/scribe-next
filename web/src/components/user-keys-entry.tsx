import { IconKey } from "@tabler/icons-react";
import { type ReactNode, createContext, useContext, useState } from "react";
import { useT } from "../i18n/i18n-context";
import { Alert, type AlertTone } from "./ui/alert";
import { Button } from "./ui/button";
import { UserKeysDialog } from "./user-keys-dialog";

/** Opens the "Key riêng" dialog; null outside `UserKeysProvider`. */
const OpenUserKeysContext = createContext<(() => void) | null>(null);

/**
 * Owns the one "Key riêng" dialog of the signed-in app, so the account
 * menu and every AI feature that needs a key open the same dialog.
 */
export function UserKeysProvider({ children }: { children: ReactNode }) {
	const [open, setOpen] = useState(false);
	return (
		<OpenUserKeysContext.Provider value={() => setOpen(true)}>
			{children}
			<UserKeysDialog open={open} onOpenChange={setOpen} />
		</OpenUserKeysContext.Provider>
	);
}

/** Opens "Key riêng"; null outside `UserKeysProvider` (nothing to open). */
export function useOpenUserKeys(): (() => void) | null {
	return useContext(OpenUserKeysContext);
}

export interface NeedsKeyNoticeProps {
	/** Why a key is needed: the feature's own text, or the server's refusal. */
	message: string;
	tone?: AlertTone;
	/** Runs before the dialog opens, e.g. to close a modal showing the notice. */
	onOpen?: () => void;
	"data-id"?: string;
}

/**
 * An AI feature without a usable own key: what is needed, and a button
 * that opens "Key riêng" where the key is added.
 */
export function NeedsKeyNotice({
	message,
	tone = "info",
	onOpen,
	"data-id": dataId = "needs-key",
}: NeedsKeyNoticeProps) {
	const t = useT();
	const openKeys = useOpenUserKeys();
	return (
		<Alert tone={tone} data-id={dataId}>
			<span>{message}</span>
			{openKeys ? (
				<Button
					size="sm"
					variant="ghost"
					className="ml-2"
					data-id={`${dataId}-open`}
					leadingIcon={<IconKey size={16} aria-hidden="true" />}
					onClick={() => {
						onOpen?.();
						openKeys();
					}}
				>
					{t("keys.needed.open")}
				</Button>
			) : null}
		</Alert>
	);
}
