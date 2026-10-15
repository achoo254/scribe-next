import { useRef, useState } from "react";
import { Button } from "../../components/ui/button";
import { FormField } from "../../components/ui/form-field";
import { Input } from "../../components/ui/input";
import { Modal, ModalClose } from "../../components/ui/modal";
import { ModalFooter } from "../../components/ui/modal-footer";
import { looksLikeApiKey } from "../../features/transcript/api-key";
import { isNativeShell } from "../../lib/api";

export interface ApiKeyModalProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	/** Why the key is asked again (e.g. OpenAI refused the saved one). */
	reason?: string;
	/** Saves the key and starts the transcription; the modal closes when it resolves. */
	onSubmit: (apiKey: string) => Promise<void>;
}

/** Asks for the user's own OpenAI API key, once per device. */
export function ApiKeyModal({
	open,
	onOpenChange,
	reason,
	onSubmit,
}: ApiKeyModalProps) {
	const input = useRef<HTMLInputElement>(null);
	const [value, setValue] = useState("");
	const [error, setError] = useState("");
	const [busy, setBusy] = useState(false);
	const where = isNativeShell()
		? "Key được lưu trong Keychain của iPhone này"
		: "Key chỉ được giữ trong tab trình duyệt này";

	function changeOpen(next: boolean): void {
		if (busy) return;
		if (!next) {
			setValue("");
			setError("");
		}
		onOpenChange(next);
	}

	async function submit(): Promise<void> {
		if (!looksLikeApiKey(value)) {
			setError("Key OpenAI bắt đầu bằng sk-.");
			return;
		}
		setBusy(true);
		setError("");
		try {
			await onSubmit(value.trim());
			setValue("");
			onOpenChange(false);
		} catch (cause) {
			setError(cause instanceof Error ? cause.message : "Không gửi được.");
		} finally {
			setBusy(false);
		}
	}

	return (
		<Modal
			open={open}
			onOpenChange={changeOpen}
			title="API key OpenAI"
			size="sm"
			initialFocusRef={input}
			data-id="transcript-key"
			footer={
				<ModalFooter>
					<ModalClose asChild>
						<Button variant="ghost" disabled={busy}>
							Huỷ
						</Button>
					</ModalClose>
					<Button
						loading={busy}
						onClick={() => void submit()}
						data-id="transcript-key-submit"
					>
						Lưu và chép lời
					</Button>
				</ModalFooter>
			}
		>
			<form
				className="grid gap-3"
				onSubmit={(event) => {
					event.preventDefault();
					void submit();
				}}
			>
				{reason ? <p className="m-0 text-error text-sm">{reason}</p> : null}
				<p className="m-0 text-muted text-sm">
					Chép lời dùng key OpenAI của chính bạn, khoảng $0,003 cho mỗi phút
					ghi. {where}; máy chủ chỉ dùng key trong lúc chép, không lưu lại.
				</p>
				<FormField label="API key" error={error}>
					<Input
						ref={input}
						type="password"
						autoComplete="off"
						spellCheck={false}
						placeholder="sk-…"
						value={value}
						onChange={(event) => setValue(event.target.value)}
						data-id="transcript-key-input"
					/>
				</FormField>
			</form>
		</Modal>
	);
}
