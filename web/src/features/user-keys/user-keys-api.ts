import { activeTranslator } from "../../i18n/active-translator";
import { apiJson } from "../../lib/api";
import {
	KEY_PROVIDERS,
	type KeyProvider,
	type KeyStatus,
	type KeyUse,
	type StoredKey,
	USE_PROVIDERS,
} from "./key-uses";

export { KEY_PROVIDERS, USE_PROVIDERS };
export type { KeyProvider, KeyStatus, KeyUse, StoredKey };

/**
 * "Key riêng": the user's own API keys, stored encrypted on the Scribe
 * server. The API is write-only: a key is sent once on save and never
 * comes back, only its provider, last four characters and status.
 */
/** The provider picked per use (null: automatic), and the text model. */
export interface KeyChoice {
	transcribe: KeyProvider | null;
	live: KeyProvider | null;
	text: KeyProvider | null;
	textModel: string | null;
}

export interface TextModel {
	id: string;
	label: string;
}

export interface UserKeys {
	/** The server can store keys (its encryption secret is in place). */
	available: boolean;
	keys: StoredKey[];
	/** What the user picked. */
	prefs: KeyChoice;
	/** What actually runs on the user's keys now (null: Scribe's key). */
	active: KeyChoice;
	/** Text models per provider, the first being the default. */
	models: Partial<Record<KeyProvider, TextModel[]>>;
}

function isProvider(value: unknown): value is KeyProvider {
	return (
		typeof value === "string" && KEY_PROVIDERS.includes(value as KeyProvider)
	);
}

function text(value: unknown): string | null {
	return typeof value === "string" && value ? value : null;
}

function field(body: unknown, name: string): unknown {
	return typeof body === "object" && body !== null
		? Reflect.get(body, name)
		: undefined;
}

const STATUSES: readonly KeyStatus[] = [
	"valid",
	"invalid",
	"quota",
	"unreadable",
];

export function parseStoredKey(body: unknown): StoredKey | null {
	const provider = field(body, "provider");
	if (!isProvider(provider)) return null;
	const status = field(body, "status");
	return {
		provider,
		last4: text(field(body, "last4")) ?? "",
		status: STATUSES.includes(status as KeyStatus)
			? (status as KeyStatus)
			: "invalid",
		updatedAt: text(field(body, "updatedAt")),
		lastCheckedAt: text(field(body, "lastCheckedAt")),
	};
}

function parseChoice(body: unknown): KeyChoice {
	const pick = (name: KeyUse): KeyProvider | null => {
		const value = field(body, name);
		return isProvider(value) && USE_PROVIDERS[name].includes(value)
			? value
			: null;
	};
	return {
		transcribe: pick("transcribe"),
		live: pick("live"),
		text: pick("text"),
		textModel: text(field(body, "textModel")),
	};
}

function parseModels(body: unknown): UserKeys["models"] {
	const out: UserKeys["models"] = {};
	for (const provider of KEY_PROVIDERS) {
		const list = field(body, provider);
		if (!Array.isArray(list)) continue;
		out[provider] = list.flatMap((item) => {
			const id = text(field(item, "id"));
			const label = text(field(item, "label"));
			return id && label ? [{ id, label }] : [];
		});
	}
	return out;
}

export function parseUserKeys(body: unknown): UserKeys {
	const keys = field(body, "keys");
	return {
		available: field(body, "available") === true,
		keys: Array.isArray(keys)
			? keys.flatMap((k) => {
					const parsed = parseStoredKey(k);
					return parsed ? [parsed] : [];
				})
			: [],
		prefs: parseChoice(field(body, "prefs")),
		active: parseChoice(field(body, "active")),
		models: parseModels(field(body, "models")),
	};
}

export async function fetchUserKeys(
	fetcher: typeof fetch = fetch,
): Promise<UserKeys> {
	const body = await apiJson<unknown>(
		"/api/me/keys",
		{ fallbackError: activeTranslator().t("errors.keys.load") },
		fetcher,
	);
	return parseUserKeys(body);
}

/** Checks `key` with its provider, then stores it (encrypted). */
export async function saveUserKey(
	provider: KeyProvider,
	key: string,
	fetcher: typeof fetch = fetch,
): Promise<StoredKey> {
	const body = await apiJson<unknown>(
		`/api/me/keys/${provider}`,
		{
			method: "PUT",
			json: { key },
			fallbackError: activeTranslator().t("errors.keys.save"),
		},
		fetcher,
	);
	const stored = parseStoredKey(body);
	if (!stored) throw new Error(activeTranslator().t("errors.keys.save"));
	return stored;
}

export async function deleteUserKey(
	provider: KeyProvider,
	fetcher: typeof fetch = fetch,
): Promise<void> {
	await apiJson<unknown>(
		`/api/me/keys/${provider}`,
		{
			method: "DELETE",
			fallbackError: activeTranslator().t("errors.keys.delete"),
		},
		fetcher,
	);
}

/** Saves what the user picked; answers like `fetchUserKeys`. */
export async function saveKeyChoice(
	choice: KeyChoice,
	fetcher: typeof fetch = fetch,
): Promise<UserKeys> {
	const body = await apiJson<unknown>(
		"/api/me/key-prefs",
		{
			method: "PUT",
			json: choice,
			fallbackError: activeTranslator().t("errors.keys.prefs"),
		},
		fetcher,
	);
	return parseUserKeys(body);
}
