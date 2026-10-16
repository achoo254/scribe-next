import { IconDeviceDesktop, IconMoon, IconSun } from "@tabler/icons-react";
import { useSyncExternalStore } from "react";
import { useT } from "../i18n/i18n-context";
import type { MessageKey } from "../i18n/message-types";
import { cn } from "../lib/cn";
import {
	type ThemePreference,
	applyThemePreference,
	readThemePreference,
	subscribeThemePreference,
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
 * Light / dark / system switch that sets `data-theme` on <html>, the same
 * control as localman-next. "system" follows `prefers-color-scheme`.
 */
export function ThemeToggle({ "data-id": dataId }: { "data-id"?: string }) {
	const t = useT();
	const preference = useSyncExternalStore(
		subscribeThemePreference,
		readThemePreference,
	);

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
					onClick={() => applyThemePreference(value)}
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
