import { IconDotsVertical } from "@tabler/icons-react";
import type { ReactNode } from "react";
import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuTrigger,
} from "./dropdown-menu";
import { IconButton } from "./icon-button";

export interface RowActionsMenuProps {
	/** Accessible name of the ⋮ button, naming the row: "Thao tác với X". */
	label: string;
	/** `data-id` of the trigger; the menu gets `<id>-content`. */
	"data-id": string;
	/** `DropdownMenuItem`s (and separators). */
	children: ReactNode;
}

/**
 * ⋮ button that opens a row's secondary actions, as in localman-next's
 * project rows: keeps narrow rows (iPhone) to one line of buttons.
 */
export function RowActionsMenu({
	label,
	"data-id": dataId,
	children,
}: RowActionsMenuProps) {
	return (
		<DropdownMenu>
			<DropdownMenuTrigger asChild>
				<IconButton
					label={label}
					icon={<IconDotsVertical size={20} aria-hidden="true" />}
					data-id={dataId}
					className="data-[state=open]:bg-fg/6 data-[state=open]:text-fg"
				/>
			</DropdownMenuTrigger>
			<DropdownMenuContent data-id={`${dataId}-content`}>
				{children}
			</DropdownMenuContent>
		</DropdownMenu>
	);
}
