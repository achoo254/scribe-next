// Hook WebView2 `PermissionRequested`: tự cấp quyền camera/mic cho getUserMedia (Phase 2 — họp video).
// WebView2 mặc định KHÔNG cho phép media → không hook = getUserMedia fail im lặng (không prompt).
// Windows-only (macOS dùng WKWebView + Info.plist NSCameraUsageDescription — out-of-scope MVP Windows-first).
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2PermissionRequestedEventArgs, COREWEBVIEW2_PERMISSION_KIND_CAMERA,
    COREWEBVIEW2_PERMISSION_KIND_MICROPHONE, COREWEBVIEW2_PERMISSION_KIND_UNKNOWN_PERMISSION,
    COREWEBVIEW2_PERMISSION_STATE_ALLOW,
};
use webview2_com::PermissionRequestedEventHandler;

/// Đăng ký handler tự ALLOW camera/mic trên webview của cửa sổ. Lỗi → trả String để caller log.
pub fn grant_media_permissions(window: &tauri::WebviewWindow) -> Result<(), String> {
    window
        .with_webview(|webview| unsafe {
            let core: ICoreWebView2 = match webview.controller().CoreWebView2() {
                Ok(c) => c,
                Err(_) => return,
            };
            let handler = PermissionRequestedEventHandler::create(Box::new(
                |_sender: Option<ICoreWebView2>,
                 args: Option<ICoreWebView2PermissionRequestedEventArgs>| {
                    if let Some(args) = args {
                        let mut kind = COREWEBVIEW2_PERMISSION_KIND_UNKNOWN_PERMISSION;
                        args.PermissionKind(&mut kind)?;
                        if kind == COREWEBVIEW2_PERMISSION_KIND_CAMERA
                            || kind == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE
                        {
                            args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
                        }
                    }
                    Ok(())
                },
            ));
            // token: *mut i64 (binding webview2-com 0.38) — không cần giữ để gỡ handler (sống cùng webview).
            let mut token: i64 = 0;
            let _ = core.add_PermissionRequested(&handler, &mut token);
        })
        .map_err(|e| e.to_string())
}
