/**
 * The Vite dev server Playwright starts for the specs. A fixed port of its
 * own: 5173/5174 belong to the developer's dev servers.
 */
export const E2E_PORT = 5180;

export const E2E_ORIGIN = `http://127.0.0.1:${E2E_PORT}`;
