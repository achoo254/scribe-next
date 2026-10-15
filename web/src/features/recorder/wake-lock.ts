/**
 * Screen Wake Lock: keeps the screen from dimming and auto-locking while
 * recording (iOS 16.4+, Chrome/Edge). It does not stop the user from
 * pressing the lock button. The browser releases it whenever the page is
 * hidden, so the recorder asks again when the page is visible. Without
 * support nothing happens and recording goes on.
 */
export class ScreenWakeLock {
	private sentinel: WakeLockSentinel | null = null;

	async acquire(): Promise<void> {
		try {
			if ("wakeLock" in navigator) {
				this.sentinel = await navigator.wakeLock.request("screen");
			}
		} catch {
			// Refused (e.g. page not visible): recording does not depend on it.
			this.sentinel = null;
		}
	}

	async release(): Promise<void> {
		const sentinel = this.sentinel;
		this.sentinel = null;
		try {
			await sentinel?.release();
		} catch {
			// Already released by the browser.
		}
	}
}
