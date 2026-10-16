import { type ReactNode, useState } from "react";
import { useToast } from "../../components/ui/toast";
import { loadApiKey, saveApiKey } from "../../features/transcript/api-key";
import { useRequestTranscripts } from "../../features/transcript/transcript-queries";
import { ApiError, errorMessage } from "../../lib/api";
import { ApiKeyModal } from "./api-key-modal";

export interface Transcribe {
	/** Queues the files with the saved key, asking for one first when there is none. */
	start: (fileIds: readonly string[]) => Promise<void>;
	/** Asks for another key (OpenAI refused this one), then queues the files. */
	askKey: (fileIds: readonly string[], reason?: string) => void;
	pending: boolean;
	/** The key dialog; render it once next to the buttons that call `start`. */
	keyModal: ReactNode;
}

/**
 * "Chép lời" with the user's own OpenAI key, shared by the recordings list
 * and the transcript reader. `onSent` runs once the backend queued the files.
 */
export function useTranscribe(onSent?: () => void): Transcribe {
	const toast = useToast();
	const request = useRequestTranscripts();
	const [prompt, setPrompt] = useState<{
		fileIds: readonly string[];
		reason?: string;
	} | null>(null);

	async function send(
		fileIds: readonly string[],
		apiKey: string,
	): Promise<void> {
		await request.mutateAsync({ fileIds, apiKey });
		onSent?.();
	}

	async function start(fileIds: readonly string[]): Promise<void> {
		const saved = await loadApiKey().catch(() => "");
		if (!saved) {
			setPrompt({ fileIds });
			return;
		}
		try {
			await send(fileIds, saved);
		} catch (cause) {
			// A malformed saved key: ask for a new one instead of failing again.
			if (
				cause instanceof ApiError &&
				cause.status === 400 &&
				/key/i.test(cause.message)
			) {
				setPrompt({ fileIds, reason: cause.message });
				return;
			}
			toast.show({
				tone: "error",
				title: "Không chép lời được",
				description: errorMessage(cause),
			});
		}
	}

	const keyModal = (
		<ApiKeyModal
			open={prompt !== null}
			onOpenChange={(next) => {
				if (!next) setPrompt(null);
			}}
			reason={prompt?.reason}
			onSubmit={async (apiKey) => {
				await saveApiKey(apiKey);
				await send(prompt?.fileIds ?? [], apiKey);
			}}
		/>
	);

	return {
		start,
		askKey: (fileIds, reason) => setPrompt({ fileIds, reason }),
		pending: request.isPending,
		keyModal,
	};
}
