import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import type { JobCriterion } from "../../features/split/split-job";
import { SplitCriteria, parseSplitValue } from "./split-criteria";

function Harness({ onCriterion }: { onCriterion: (c: JobCriterion) => void }) {
	const [criterion, setCriterion] = useState<JobCriterion>("minutes");
	const [value, setValue] = useState("10");
	return (
		<SplitCriteria
			criterion={criterion}
			onCriterionChange={(next) => {
				onCriterion(next);
				setCriterion(next);
			}}
			value={value}
			onValueChange={setValue}
			disabled={false}
		/>
	);
}

describe("SplitCriteria", () => {
	it("picks the criterion from a select and labels the value for it", async () => {
		const onCriterion = vi.fn();
		render(<Harness onCriterion={onCriterion} />);
		const select = screen.getByRole("combobox", { name: "Cắt theo" });
		expect(select).toHaveValue("minutes");
		expect(screen.getByLabelText("Số phút mỗi đoạn")).toHaveValue(10);

		await userEvent.selectOptions(select, "Số phần bằng nhau");
		expect(onCriterion).toHaveBeenCalledWith("parts");
		const parts = screen.getByLabelText("Chia thành (số đoạn)");
		await userEvent.clear(parts);
		await userEvent.type(parts, "2.5");
		expect(
			screen.getByText("Nhập số nguyên, từ 1 trở lên."),
		).toBeInTheDocument();
	});

	it("parses whole parts and positive amounts only", () => {
		expect(parseSplitValue("parts", "3")).toBe(3);
		expect(parseSplitValue("parts", "2.5")).toBeNull();
		expect(parseSplitValue("megabytes", "0.5")).toBe(0.5);
		expect(parseSplitValue("minutes", "0")).toBeNull();
	});
});
