import { IconSearch } from "@tabler/icons-react";
import { useState } from "react";
import { useNavigate } from "react-router";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { useT } from "../../i18n/i18n-context";
import { searchPath } from "../../layout/app-paths";

/**
 * Search field of "Lời đã chép": "Tìm" (or Enter) opens the search page
 * with what was typed, where results follow the typing and "Hỏi Scribe" is.
 */
export function SearchEntry() {
	const t = useT();
	const navigate = useNavigate();
	const [draft, setDraft] = useState("");
	return (
		<form
			data-id="search-entry"
			className="flex gap-2"
			onSubmit={(event) => {
				event.preventDefault();
				navigate(searchPath(draft));
			}}
		>
			<div className="relative min-w-0 flex-1">
				<IconSearch
					size={18}
					aria-hidden="true"
					className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-icon-muted"
				/>
				<Input
					type="search"
					data-id="search-entry-input"
					aria-label={t("search.entry.label")}
					placeholder={t("search.entry.placeholder")}
					value={draft}
					maxLength={200}
					className="pl-10"
					onChange={(event) => setDraft(event.target.value)}
				/>
			</div>
			<Button type="submit" data-id="search-entry-submit">
				{t("search.entry.submit")}
			</Button>
		</form>
	);
}
