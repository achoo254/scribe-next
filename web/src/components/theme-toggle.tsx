import { useThemePreference as useSharedThemePreference } from "@achoo254/app-shell";
import { IconDeviceDesktop, IconMoon, IconSun } from "@tabler/icons-react";
import { useT } from "../i18n/i18n-context";
import type { MessageKey } from "../i18n/message-types";
import { cn } from "../lib/cn";
import {
	PREFERENCE_OPTIONS,
	type ThemePreference,
	pickTheme,
} from "../lib/theme";

const OPTIONS = [
	{ value: "light", label: "ui.theme.light", Icon: IconSun },
	{ value: "dark", label: "ui.theme.dark", Icon: IconMoon },
	{ value: "system", label: "ui.theme.system", Icon: IconDeviceDesktop },
] as const satisfies ReadonlyArray<{
	value: ThemePreference;
	label: MessageKey;
	Icon: typeof IconSun;
}>;

/**
 * Light / dark / system switch of the signed-in footer, the same control as
 * localman-next. It reads and writes the shared choice of every hoandat.com
 * app (`lib/theme.ts`), the one the footer of the signed-out pages
 * (@achoo254/app-shell `ThemeSwitch`) sets. "system" follows
 * `prefers-color-scheme`.
 */
export function ThemeToggle({ "data-id": dataId }: { "data-id"?: string }) {
	const t = useT();
	const preference = useThemePreference();

	return (
		<fieldset
			data-id={dataId}
			className="m-0 inline-flex gap-1 rounded-md border border-border bg-surface p-1"
		>
			<legend className="sr-only">{t("ui.theme.legend")}</legend>
			{OPTIONS.map(({ value, label, Icon }) => (
				<button
					key={value}
					type="button"
					aria-label={t(label)}
					aria-pressed={preference === value}
					title={t(label)}
					data-id={dataId ? `${dataId}-${value}` : undefined}
					onClick={() => pickTheme(value)}
					className={cn(
						"inline-flex size-8 cursor-pointer items-center justify-center rounded-sm transition-colors pointer-coarse:size-11",
						preference === value
							? "bg-primary-solid text-on-primary"
							: "text-muted hover:bg-fg/6 hover:text-fg",
					)}
				>
					<Icon size={16} aria-hidden="true" />
				</button>
			))}
		</fieldset>
	);
}

function useThemePreference(): ThemePreference {
	return useSharedThemePreference(PREFERENCE_OPTIONS);
}

/**
 * One-button form of the switch for narrow screens: shows the current choice
 * and moves to the next one (light → dark → system) on each tap.
 */
export function ThemeCycleButton({
	"data-id": dataId,
	className,
}: { "data-id"?: string; className?: string }) {
	const t = useT();
	const preference = useThemePreference();
	const index = OPTIONS.findIndex((o) => o.value === preference);
	const current = OPTIONS[index] ?? OPTIONS[2];
	const next = OPTIONS[(index + 1) % OPTIONS.length] ?? OPTIONS[0];
	const label = t("ui.theme.cycle", { current: t(current.label) });
	return (
		<button
			type="button"
			data-id={dataId}
			aria-label={label}
			title={label}
			onClick={() => pickTheme(next.value)}
			className={cn(
				"inline-flex size-11 cursor-pointer items-center justify-center rounded-md border border-border bg-surface text-muted transition-colors hover:text-fg",
				className,
			)}
		>
			<current.Icon size={18} aria-hidden="true" />
		</button>
	);
}
