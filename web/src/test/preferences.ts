/** Forgets the shared choices (`hd_theme`, `hd_lang`) and the old theme key between tests. */
export function clearPreferences(): void {
	for (const name of ["hd_theme", "hd_lang"]) {
		document.cookie = `${name}=; Max-Age=0; Path=/`;
	}
	window.localStorage.clear();
	delete document.documentElement.dataset.theme;
}
