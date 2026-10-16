import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useRef, useSyncExternalStore } from "react";
import { useAuth } from "../../auth/auth-context";
import { useToast } from "../../components/ui/toast";
import { errorMessage } from "../../lib/api";
import { refreshStorage } from "../files/file-queries";
import {
	NativeRecorder,
	NativeRecorderPlugin,
	type NativeRecorderState,
	userKeyOf,
} from "./native-recorder";

export interface NativeRecorderApi extends NativeRecorderState {
	recorder: NativeRecorder;
	/** Per-account key, null until derived from the signed-in email. */
	userKey: string | null;
}

/**
 * The Swift recorder bound to React: attaches its events, re-reads its
 * status when the page becomes visible, and on mount recovers what a
 * killed session left behind and resumes uploads for this account. Used
 * once, by `RecordStateProvider` (signed-in session), so the upload sync
 * runs once per sign-in rather than on every visit of /record.
 */
export function useNativeRecorder(): NativeRecorderApi {
	const queryClient = useQueryClient();
	const toast = useToast();
	const { user, logout } = useAuth();
	const ref = useRef<NativeRecorder | null>(null);
	const keyRef = useRef<string | null>(null);
	const latest = useRef({ toast, logout, queryClient });
	latest.current = { toast, logout, queryClient };

	ref.current ??= new NativeRecorder(NativeRecorderPlugin, {
		onUploaded: () =>
			void refreshStorage(latest.current.queryClient, ["recordings"]),
		onNeedsSignIn: () => {
			latest.current.toast.show({
				tone: "info",
				title: "Phiên đăng nhập đã hết",
				description: "Đăng nhập lại để tải nốt các đoạn đang chờ trên máy.",
			});
			// Never interrupt a recording: segments wait on the phone, and
			// signing out (which stops the recorder) can happen after it ends.
			if (!ref.current?.getState().recording) void latest.current.logout();
		},
		onQuotaFull: (message) =>
			latest.current.toast.show({
				tone: "error",
				title: "Hết dung lượng",
				description: `${message} Các đoạn vẫn giữ trên máy và tự tải lên khi có chỗ.`,
			}),
	});
	const recorder = ref.current;
	const state = useSyncExternalStore(recorder.subscribe, recorder.getState);

	useEffect(() => {
		void recorder.attach();
		const onVisible = () => {
			if (document.visibilityState === "visible") void recorder.refresh();
		};
		document.addEventListener("visibilitychange", onVisible);
		return () => {
			document.removeEventListener("visibilitychange", onVisible);
			recorder.dispose();
		};
	}, [recorder]);

	const email = user?.email ?? null;
	useEffect(() => {
		if (!email) return;
		let cancelled = false;
		userKeyOf(email)
			.then(async (key) => {
				if (cancelled) return;
				keyRef.current = key;
				const { lostSegments, lostSecondsMax } =
					await NativeRecorderPlugin.syncUploads({ userKey: key });
				if (lostSegments > 0) {
					latest.current.toast.show({
						tone: "error",
						title: "Mất phần cuối của một bản ghi",
						description: `Ứng dụng bị đóng khi đang ghi: mất tối đa ${Math.round(lostSecondsMax)} giây của đoạn đang mở.`,
						duration: 10_000,
					});
				}
			})
			.catch((cause: unknown) =>
				latest.current.toast.show({
					tone: "error",
					title: "Chưa đồng bộ được các đoạn ghi trên máy",
					description: errorMessage(cause, "Mở lại ứng dụng để thử lại."),
				}),
			);
		return () => {
			cancelled = true;
		};
	}, [email]);

	return { ...state, recorder, userKey: keyRef.current };
}
