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

/** Longer than the slowest motion token so the exit animation can finish. */
const EXIT_DELAY_MS = 400;

export type ToastTone = "success" | "error" | "info";

export interface ToastOptions {
	title: ReactNode;
	description?: ReactNode;
	tone?: ToastTone;
	/** Milliseconds before auto-dismiss; Radix pauses it on hover/focus. */
	duration?: number;
}

interface ToastItem extends ToastOptions {
	id: number;
}

interface ToastApi {
	show: (options: ToastOptions) => void;
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
	const [items, setItems] = useState<ToastItem[]>([]);
	const nextId = useRef(0);

	const show = useCallback((options: ToastOptions) => {
		nextId.current += 1;
		const id = nextId.current;
		setItems((current) => [...current, { ...options, id }]);
	}, []);

	const remove = useCallback((id: number) => {
		setItems((current) => current.filter((item) => item.id !== id));
	}, []);

	const api = useMemo<ToastApi>(() => ({ show }), [show]);

	return (
		<ToastContext.Provider value={api}>
			<RadixToast.Provider label={label ?? "Thông báo"} swipeDirection="down">
				{children}
				{items.map(({ id, title, description, tone = "success", duration }) => {
					const { tone: toneClass, Icon } = TONES[tone];
					return (
						<RadixToast.Root
							key={id}
							duration={duration ?? 5000}
							data-tone={tone}
							onOpenChange={(open) => {
								// Radix plays the exit animation once the toast closes;
								// drop it from state only after that has finished.
								if (!open) window.setTimeout(() => remove(id), EXIT_DELAY_MS);
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
							<RadixToast.Close
								aria-label={closeLabel ?? "Đóng thông báo"}
								className="-m-1 cursor-pointer rounded-xs p-1 text-icon-muted hover:text-fg"
							>
								<IconX size={16} aria-hidden="true" />
							</RadixToast.Close>
						</RadixToast.Root>
					);
				})}
				<RadixToast.Viewport className="-translate-x-1/2 fixed bottom-6 left-1/2 z-200 m-0 grid w-[min(440px,calc(100vw-2*var(--gutter)))] list-none gap-2 p-0 outline-none" />
			</RadixToast.Provider>
		</ToastContext.Provider>
	);
}
