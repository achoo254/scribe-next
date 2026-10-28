import { ThemeSwitch } from "@achoo254/app-shell";
import { IconInbox, IconMicrophone, IconTrash } from "@tabler/icons-react";
import { useState } from "react";
import { BrandMark, Wordmark } from "../../components/brand/wordmark";
import { Alert } from "../../components/ui/alert";
import { Badge } from "../../components/ui/badge";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { ConfirmModal } from "../../components/ui/confirm-modal";
import { EmptyState } from "../../components/ui/empty-state";
import { FormField } from "../../components/ui/form-field";
import { IconButton } from "../../components/ui/icon-button";
import { Input } from "../../components/ui/input";
import { Modal } from "../../components/ui/modal";
import { PanelProgress } from "../../components/ui/panel-progress";
import { SegmentedControl } from "../../components/ui/segmented-control";
import { Skeleton } from "../../components/ui/skeleton";
import { SwitchField } from "../../components/ui/switch-field";
import {
	Tabs,
	TabsContent,
	TabsList,
	TabsTrigger,
} from "../../components/ui/tabs";
import { useToast } from "../../components/ui/toast";
import { Tooltip } from "../../components/ui/tooltip";
import { useI18n } from "../../i18n/i18n-context";
import { PRIMARY_TENANT_DOMAIN } from "../../lib/brand";
import { LEGACY_THEME_KEY } from "../../lib/theme-bootstrap";

type Unit = "minutes" | "megabytes" | "parts";

/** Development-only page listing every primitive, to review both themes. */
export function UiGalleryPage() {
	const { locale } = useI18n();
	const toast = useToast();
	const [checked, setChecked] = useState(true);
	const [unit, setUnit] = useState<Unit>("minutes");
	const [confirmOpen, setConfirmOpen] = useState(false);
	const [refreshing, setRefreshing] = useState(false);

	return (
		<main
			data-id="ui-gallery"
			className="mx-auto grid max-w-page gap-6 px-gutter py-6"
		>
			<header className="flex flex-wrap items-center justify-between gap-3">
				<span className="flex items-center gap-2">
					<BrandMark size={32} />
					<Wordmark />
				</span>
				<ThemeSwitch
					locale={locale}
					domain={PRIMARY_TENANT_DOMAIN}
					legacyThemeKey={LEGACY_THEME_KEY}
				/>
			</header>

			<Card className="grid gap-4 p-6">
				<h2 className="m-0 font-bold font-mono text-lg">Button</h2>
				<div className="flex flex-wrap gap-3">
					<Button variant="primary" leadingIcon={<IconMicrophone size={18} />}>
						Bắt đầu ghi
					</Button>
					<Button>Secondary</Button>
					<Button variant="ghost">Ghost</Button>
					<Button variant="danger">Xoá</Button>
					<Button variant="primary" loading>
						Đang lưu
					</Button>
					<Button disabled>Disabled</Button>
					<IconButton
						label="Xoá"
						icon={<IconTrash size={18} aria-hidden="true" />}
					/>
					<Tooltip content="Tooltip đảo màu">
						<Button size="sm">Di chuột</Button>
					</Tooltip>
				</div>
			</Card>

			<Card className="grid gap-4 p-6">
				<h2 className="m-0 font-bold font-mono text-lg">Badge và Alert</h2>
				<div className="flex flex-wrap gap-2">
					{(
						[
							"neutral",
							"primary",
							"accent",
							"info",
							"success",
							"warning",
							"danger",
						] as const
					).map((tone) => (
						<Badge key={tone} tone={tone}>
							{tone}
						</Badge>
					))}
				</div>
				<Alert tone="info" title="Thông tin">
					Bản ghi được tự động xoá sau 14 ngày.
				</Alert>
				<Alert tone="success">Đã tải lên.</Alert>
				<Alert tone="warning">Hãy bật ít nhất một nguồn ghi.</Alert>
				<Alert tone="danger">Trình duyệt chưa được cấp quyền Micro.</Alert>
			</Card>

			<Card className="grid gap-4 p-6">
				<h2 className="m-0 font-bold font-mono text-lg">Form</h2>
				<FormField label="Số phút mỗi đoạn" hint="Từ 1 trở lên">
					<Input type="number" defaultValue={10} />
				</FormField>
				<FormField label="Có lỗi" error="Giá trị không hợp lệ">
					<Input defaultValue="abc" />
				</FormField>
				<SwitchField
					label="Micro"
					checked={checked}
					onCheckedChange={setChecked}
				/>
				<SegmentedControl
					legend="Tiêu chí"
					data-id="ui-gallery-unit"
					value={unit}
					onChange={setUnit}
					options={[
						{ value: "minutes", label: "Phút", "data-id": "unit-minutes" },
						{ value: "megabytes", label: "MB", "data-id": "unit-megabytes" },
						{ value: "parts", label: "Số phần", "data-id": "unit-parts" },
					]}
				/>
			</Card>

			<Card className="overflow-hidden">
				<Tabs defaultValue="a">
					<TabsList>
						<TabsTrigger value="a">Ghi âm</TabsTrigger>
						<TabsTrigger value="b">Cắt file</TabsTrigger>
						<TabsTrigger value="c" disabled>
							Transcript
						</TabsTrigger>
					</TabsList>
					<TabsContent value="a" className="p-6">
						Nội dung tab Ghi âm
					</TabsContent>
					<TabsContent value="b" className="p-6">
						Nội dung tab Cắt file
					</TabsContent>
				</Tabs>
			</Card>

			<Card className="grid gap-4 p-6">
				<h2 className="m-0 font-bold font-mono text-lg">Modal và Toast</h2>
				<div className="flex flex-wrap gap-3">
					<Modal
						title="Modal"
						description="Mô tả ngắn."
						trigger={<Button>Mở modal</Button>}
					>
						<p className="m-0">Nội dung modal.</p>
					</Modal>
					<Button variant="danger" onClick={() => setConfirmOpen(true)}>
						Mở xác nhận xoá
					</Button>
					<Button
						onClick={() =>
							toast.show({ title: "Đã lưu bản ghi", description: "Bản ghi 1" })
						}
					>
						Toast
					</Button>
				</div>
				<ConfirmModal
					open={confirmOpen}
					onOpenChange={setConfirmOpen}
					title="Xoá bản ghi"
					question="Xoá “Bản ghi 1”?"
					confirmLabel="Xoá"
					onConfirm={async () => {}}
				>
					Hành động này không thể hoàn tác.
				</ConfirmModal>
			</Card>

			<Card className="grid gap-4 p-6">
				<h2 className="m-0 font-bold font-mono text-lg">Tải dữ liệu</h2>
				<Skeleton variant="text" lines={3} className="max-w-md" />
				<Button size="sm" onClick={() => setRefreshing((value) => !value)}>
					{refreshing ? "Dừng làm mới" : "Làm mới"}
				</Button>
				<PanelProgress
					active={refreshing}
					aria-busy={refreshing}
					className="overflow-hidden rounded-md border border-border"
				>
					<p className="m-0 p-4">Danh sách giữ nguyên khi làm mới.</p>
				</PanelProgress>
				<EmptyState icon={<IconInbox size={28} />} title="Chưa có bản ghi">
					Bản ghi mới sẽ hiện ở đây.
				</EmptyState>
			</Card>
		</main>
	);
}
