import * as RadixTabs from "@radix-ui/react-tabs";
import type { ComponentProps } from "react";
import { cn } from "../../lib/cn";

/**
 * Tabs on Radix: `tablist`/`tab`/`tabpanel` roles, ←/→, Home/End, and
 * hidden panels out of the focus order. Controlled through `value` and
 * `onValueChange` when the choice lives in the URL.
 */
export const Tabs = RadixTabs.Root;

/** Row of triggers over a bottom rule; scrolls sideways when too narrow. */
export function TabsList({
	className,
	...rest
}: ComponentProps<typeof RadixTabs.List>) {
	return (
		<RadixTabs.List
			className={cn(
				"flex overflow-x-auto overflow-y-hidden border-border-subtle border-b px-4 pt-2",
				className,
			)}
			{...rest}
		/>
	);
}

/** Mono label, at least 44px tall; the selected one is primary, underlined. */
export function TabsTrigger({
	className,
	...rest
}: ComponentProps<typeof RadixTabs.Trigger>) {
	return (
		<RadixTabs.Trigger
			className={cn(
				"-mb-px inline-flex min-h-11 cursor-pointer items-center whitespace-nowrap border-transparent border-b-2 bg-transparent px-3",
				"font-mono font-semibold text-[0.8125rem] text-muted transition-colors duration-(--dur-fast)",
				"hover:text-fg data-[state=active]:border-primary data-[state=active]:text-primary",
				className,
			)}
			{...rest}
		/>
	);
}

export function TabsContent({
	className,
	...rest
}: ComponentProps<typeof RadixTabs.Content>) {
	return <RadixTabs.Content className={cn("min-w-0", className)} {...rest} />;
}
