import * as RadixToast from "@radix-ui/react-toast";
import {
	IconAlertCircle,
	IconCircleCheck,
	IconInfoCircle,
	IconX,
} from "@tabler/icons-react";
import {
	type ReactNode,
	createContext,
	useCallback,
	useContext,
	useMemo,
	useRef,
	useState,
} from "react";
import { useT } from "../../i18n/i18n-context";

/** Longer than the slowest motion token so the exit animation can finish. */
const EXIT_DELAY_MS = 400;

export type ToastTone = "success" | "error" | "info";

export interface ToastOptions {
	title: ReactNode;
	description?: ReactNode;
	tone?: ToastTone;
	/** Milliseconds before auto-dismiss; Radix pauses it on hover/focus. */
	duration?: number;
	/** One button in the toast (e.g. "Hoàn tác"); pressing it closes the toast. */
	action?: { label: string; onClick: () => void };
	/** Called once when the toast closes, whatever closed it (after `action`). */
	onClose?: () => void;
}

interface ToastItem extends ToastOptions {
	id: number;
	open: boolean;
}

interface ToastApi {
	/** Shows a toast; returns its id for `dismiss`. */
	show: (options: ToastOptions) => number;
	/** Closes a toast early (runs its `onClose`); unknown ids are ignored. */
	dismiss: (id: number) => void;
}

const ToastContext = createContext<ToastApi | null>(null);

const TONES: Record<ToastTone, { tone: string; Icon: typeof IconInfoCircle }> =
	{
		success: { tone: "[--tone:var(--c-success)]", Icon: IconCircleCheck },
		error: { tone: "[--tone:var(--c-error)]", Icon: IconAlertCircle },
		info: { tone: "[--tone:var(--c-info)]", Icon: IconInfoCircle },
	};

/** Returns `show()`; must be used under <ToastProvider>. */
export function useToast(): ToastApi {
	const api = useContext(ToastContext);
	if (!api) throw new Error("useToast must be used inside <ToastProvider>");
	return api;
}

export interface ToastProviderProps {
	children: ReactNode;
	/** Accessible name of the notification region. */
	label?: string;
	closeLabel?: string;
}

export function ToastProvider({
	children,
	label,
	closeLabel,
}: ToastProviderProps) {
	const t = useT();
	const [items, setItems] = useState<ToastItem[]>([]);
	const nextId = useRef(0);

	// `onClose` of open toasts, by id: taken out on the first close so it
	// runs exactly once.
	const closers = useRef(new Map<number, (() => void) | undefined>());

	const show = useCallback((options: ToastOptions) => {
		nextId.current += 1;
		const id = nextId.current;
		closers.current.set(id, options.onClose);
		setItems((current) => [...current, { ...options, id, open: true }]);
		return id;
	}, []);

	const remove = useCallback((id: number) => {
		setItems((current) => current.filter((item) => item.id !== id));
	}, []);

	const dismiss = useCallback(
		(id: number) => {
			if (!closers.current.has(id)) return;
			const onClose = closers.current.get(id);
			closers.current.delete(id);
			setItems((current) =>
				current.map((item) =>
					item.id === id ? { ...item, open: false } : item,
				),
			);
			// Radix plays the exit animation once the toast closes; drop it
			// from state only after that has finished.
			window.setTimeout(() => remove(id), EXIT_DELAY_MS);
			onClose?.();
		},
		[remove],
	);

	const api = useMemo<ToastApi>(() => ({ show, dismiss }), [show, dismiss]);

	return (
		<ToastContext.Provider value={api}>
			<RadixToast.Provider
				label={label ?? t("ui.toast.region")}
				swipeDirection="down"
			>
				{children}
				{items.map(
					({
						id,
						open,
						title,
						description,
						tone = "success",
						duration,
						action,
					}) => {
						const { tone: toneClass, Icon } = TONES[tone];
						return (
							<RadixToast.Root
								key={id}
								open={open}
								duration={duration ?? 5000}
								data-tone={tone}
								onOpenChange={(next) => {
									if (!next) dismiss(id);
								}}
								className={`${toneClass} pointer-events-auto flex items-start gap-2 rounded-md border border-border border-l-3 border-l-(--tone) bg-surface px-4 py-3 text-fg text-sm shadow-lg data-[state=closed]:animate-fade-out data-[state=open]:animate-toast-in`}
							>
								<Icon
									size={18}
									aria-hidden="true"
									className="mt-px text-(--tone)"
								/>
								<div className="min-w-0 flex-1">
									<RadixToast.Title className="font-medium">
										{title}
									</RadixToast.Title>
									{description ? (
										<RadixToast.Description className="text-muted">
											{description}
										</RadixToast.Description>
									) : null}
								</div>
								{action ? (
									<RadixToast.Action
										altText={action.label}
										className="-my-1 cursor-pointer rounded-xs px-1 py-1 font-semibold text-primary hover:underline"
										onClick={action.onClick}
									>
										{action.label}
									</RadixToast.Action>
								) : null}
								<RadixToast.Close
									aria-label={closeLabel ?? t("ui.toast.close")}
									className="-m-1 cursor-pointer rounded-xs p-1 text-icon-muted hover:text-fg"
								>
									<IconX size={16} aria-hidden="true" />
								</RadixToast.Close>
							</RadixToast.Root>
						);
					},
				)}
				<RadixToast.Viewport className="-translate-x-1/2 fixed bottom-6 left-1/2 z-200 m-0 grid w-[min(440px,calc(100vw-2*var(--gutter)))] list-none gap-2 p-0 outline-none" />
			</RadixToast.Provider>
		</ToastContext.Provider>
	);
}
