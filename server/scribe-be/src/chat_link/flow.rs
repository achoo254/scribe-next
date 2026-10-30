//! Hội thoại liên kết trong chat riêng với bot, tách khỏi Mongo và Bot API để kiểm được bằng store giả.
//!
//! Chống liên kết hộ (kẻ xấu gửi đường dẫn của tài khoản mình cho người khác bấm, để biên bản của kẻ xấu
//! chảy vào chat nạn nhân hoặc ngược lại): `/start <token>` chưa liên kết gì, bot chỉ gắn token với chat
//! đó và hỏi lại, nêu tài khoản Scribe (email che bớt) kèm nút "Xác nhận liên kết" / "Huỷ". Chỉ lần bấm
//! "Xác nhận" từ đúng chat đó mới dùng token và liên kết. Liên kết xong bot nhắc `/unlink` để gỡ; liên
//! kết sang chat khác thì chat cũ được báo.

use crate::error::AppError;
use crate::lang::Lang;
pub use crate::models::chat_link::TokenOwner;
use mongodb::bson::{doc, Document};
use std::future::Future;

/// Chữ bot viết trong luồng liên kết, theo [`reply_lang`]: ngôn ngữ lưu trong token ([`TokenOwner::lang`]), không có
/// thì ngôn ngữ giao diện chat của người gửi (`from.language_code`), cuối cùng tiếng Việt.
#[derive(Debug, PartialEq)]
pub struct Texts {
    pub linked: &'static str,
    pub guide: &'static str,
    pub blocked: &'static str,
    /// Token chưa bị dùng: thử lại chính đường dẫn đó được.
    pub later: &'static str,
    /// Token đã bị dùng ở bước xác nhận: đường dẫn cũ không dùng lại được nữa.
    pub retry_new: &'static str,
    pub cancelled: &'static str,
    pub unlinked: &'static str,
    pub not_linked: &'static str,
    pub moved: &'static str,
    pub stale_button: &'static str,
    pub confirm_button: &'static str,
    pub cancel_button: &'static str,
    /// Trả lời lần bấm nút (hiện thoáng qua).
    pub pressed_cancel: &'static str,
    pub pressed_failed: &'static str,
    pub pressed_linked: &'static str,
}

const VI: Texts = Texts {
    linked: "Đã liên kết với Scribe. Biên bản bạn gửi từ Scribe sẽ tới chat này. Gửi /unlink nếu muốn gỡ liên kết.",
    guide: "Liên kết không hợp lệ hoặc đã hết hạn. Mở Scribe, chọn \"Liên kết chat\" rồi bấm lại đường dẫn mới (đường dẫn dùng được 10 phút).",
    blocked: "Tài khoản Scribe này đang bị khoá nên không liên kết được.",
    later: "Scribe đang bận, thử lại sau ít phút.",
    retry_new: "Không liên kết được lúc này. Tạo đường dẫn mới trong Scribe rồi thử lại.",
    cancelled: "Đã huỷ, chat này không được liên kết.",
    unlinked: "Đã gỡ liên kết. Scribe sẽ không gửi biên bản tới chat này nữa.",
    not_linked: "Chat này chưa liên kết với tài khoản Scribe nào.",
    moved: "Tài khoản Scribe của bạn vừa được liên kết với một chat khác; chat này không nhận biên bản nữa.",
    stale_button: "Nút này không còn dùng được.",
    confirm_button: "Xác nhận liên kết",
    cancel_button: "Huỷ",
    pressed_cancel: "Đã huỷ.",
    pressed_failed: "Không liên kết được.",
    pressed_linked: "Đã liên kết.",
};

const EN: Texts = Texts {
    linked: "Linked to Scribe. Minutes you send from Scribe will arrive in this chat. Send /unlink to unlink.",
    guide: "This link is invalid or has expired. Open Scribe, choose \"Link chat.hoandat.com\" and open the new link (a link works for 10 minutes).",
    blocked: "This Scribe account is locked, so it can't be linked.",
    later: "Scribe is busy, try again in a few minutes.",
    retry_new: "Couldn't link right now. Create a new link in Scribe and try again.",
    cancelled: "Cancelled; this chat was not linked.",
    unlinked: "Unlinked. Scribe will no longer send minutes to this chat.",
    not_linked: "This chat isn't linked to any Scribe account.",
    moved: "Your Scribe account was just linked to another chat; this chat no longer receives minutes.",
    stale_button: "This button no longer works.",
    confirm_button: "Confirm link",
    cancel_button: "Cancel",
    pressed_cancel: "Cancelled.",
    pressed_failed: "Couldn't link.",
    pressed_linked: "Linked.",
};

/// Thuật ngữ khớp bản tiếng Trung của web (`web/src/i18n/messages/zh-CN/chat.ts`).
const ZH: Texts = Texts {
    linked: "已关联 Scribe。你从 Scribe 发送的会议纪要会发到这个聊天。如需取消关联，请发送 /unlink。",
    guide: "链接无效或已过期。请打开 Scribe，选择“关联 chat.hoandat.com”，然后打开新的链接（链接 10 分钟内有效）。",
    blocked: "这个 Scribe 账号已被锁定，无法关联。",
    later: "Scribe 正忙，请几分钟后重试。",
    retry_new: "暂时无法关联。请在 Scribe 中生成新的链接后重试。",
    cancelled: "已取消，这个聊天没有被关联。",
    unlinked: "已取消关联。Scribe 不会再向这个聊天发送会议纪要。",
    not_linked: "这个聊天没有关联任何 Scribe 账号。",
    moved: "你的 Scribe 账号刚刚关联到了另一个聊天，这个聊天将不再收到会议纪要。",
    stale_button: "这个按钮已失效。",
    confirm_button: "确认关联",
    cancel_button: "取消",
    pressed_cancel: "已取消。",
    pressed_failed: "无法关联。",
    pressed_linked: "已关联。",
};

/// Ngôn ngữ bot trả lời một người. `stored` = ngôn ngữ lưu cùng token (người đó chọn trong Scribe lúc tạo đường dẫn,
/// nên được ưu tiên và giữ cả luồng liên kết một ngôn ngữ); `chat` = ngôn ngữ giao diện chat của người gửi, dùng khi
/// Scribe chưa biết người đó (không có token, token lạ/hết hạn, nút cũ, `/unlink`) hoặc token không lưu ngôn ngữ.
pub fn reply_lang(stored: Option<Lang>, chat: Option<Lang>) -> Lang {
    stored.or(chat).unwrap_or_default()
}

pub fn texts(lang: Lang) -> &'static Texts {
    match lang {
        Lang::Vi => &VI,
        Lang::En => &EN,
        Lang::Zh => &ZH,
    }
}

/// Câu hỏi lại trước khi liên kết, nêu tài khoản (email đã che) và tên nút xác nhận.
fn confirm_question(lang: Lang, masked_email: &str) -> String {
    let button = texts(lang).confirm_button;
    match lang {
        Lang::Vi => format!(
            "Liên kết chat này với tài khoản Scribe {masked_email}?\nChỉ bấm \"{button}\" nếu đây là tài khoản của bạn và bạn vừa mở đường dẫn từ Scribe."
        ),
        Lang::En => format!(
            "Link this chat to the Scribe account {masked_email}?\nOnly press \"{button}\" if this is your account and you just opened the link from Scribe."
        ),
        Lang::Zh => format!(
            "将这个聊天关联到 Scribe 账号 {masked_email}？\n只有在这是你的账号、并且你刚从 Scribe 打开链接时，才点击“{button}”。"
        ),
    }
}

const CONFIRM_PREFIX: &str = "link:";
const CANCEL_PREFIX: &str = "cancel:";

/// Một tài khoản Scribe, đủ để hỏi xác nhận.
#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    pub email: String,
    pub blocked: bool,
}

/// Phần dữ liệu luồng liên kết cần (Mongo ở `poll`, bản giả trong test).
pub trait Store {
    /// Gắn token còn hạn với chat (chat đầu tiên gửi nó); trả user (kèm ngôn ngữ) của token.
    fn bind_token(&self, token: &str, chat_id: i64) -> impl Future<Output = Result<Option<TokenOwner>, AppError>> + Send;
    /// Dùng token đã gắn với đúng chat này (một lần); trả user (kèm ngôn ngữ) nếu còn hạn.
    fn take_token(&self, token: &str, chat_id: i64) -> impl Future<Output = Result<Option<TokenOwner>, AppError>> + Send;
    /// Bỏ token đã gắn với chat này; trả ngôn ngữ của token khi đã bỏ.
    fn drop_token(&self, token: &str, chat_id: i64) -> impl Future<Output = Result<Option<Lang>, AppError>> + Send;
    fn account(&self, user_id: &str) -> impl Future<Output = Result<Option<Account>, AppError>> + Send;
    /// Gắn chat (kèm tên người bấm xác nhận, để Scribe nêu chat đã liên kết). `None` khi không còn user;
    /// `Some(chat cũ)` khi đã gắn.
    fn link(
        &self,
        user_id: &str,
        chat_id: i64,
        chat_name: Option<&str>,
    ) -> impl Future<Output = Result<Option<Option<i64>>, AppError>> + Send;
    /// Gỡ mọi liên kết tới chat; trả user đã gỡ.
    fn unlink_chat(&self, chat_id: i64) -> impl Future<Output = Result<Vec<String>, AppError>> + Send;
}

/// Việc cần làm sau một update, theo thứ tự.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Gửi tin, kèm một hàng nút `(nhãn, callback_data)` nếu có.
    Reply { chat_id: i64, text: String, buttons: Vec<(String, String)> },
    /// Trả lời lần bấm nút.
    Answer { callback_id: String, text: String },
    /// Gỡ nút của tin bot đã gửi (đã bấm xong).
    ClearButtons { chat_id: i64, seq: i64 },
    Audit { user_id: String, action: &'static str, details: Document },
}

fn reply(chat_id: i64, text: &str) -> Effect {
    Effect::Reply { chat_id, text: text.to_string(), buttons: Vec::new() }
}

fn answer(callback_id: &str, text: &str) -> Effect {
    Effect::Answer { callback_id: callback_id.to_string(), text: text.to_string() }
}

/// Email che bớt để người dùng nhận ra tài khoản mà người khác không đọc được đủ: `a***@domain`.
pub fn mask_email(email: &str) -> String {
    match email.split_once('@') {
        Some((local, domain)) if !local.is_empty() && !domain.is_empty() => {
            let first: String = local.chars().take(1).collect();
            format!("{first}***@{domain}")
        }
        _ => "***".to_string(),
    }
}

/// `/start <token>` từ người dùng có ngôn ngữ giao diện chat `chat_lang`: hỏi lại trước khi liên kết.
pub async fn on_start(store: &impl Store, chat_id: i64, token: Option<&str>, chat_lang: Option<Lang>) -> Vec<Effect> {
    let unknown = texts(reply_lang(None, chat_lang));
    let Some(token) = token else { return vec![reply(chat_id, unknown.guide)] };
    let TokenOwner { user_id, lang } = match store.bind_token(token, chat_id).await {
        Ok(Some(owner)) => owner,
        Ok(None) => return vec![reply(chat_id, unknown.guide)],
        Err(e) => {
            tracing::warn!("liên kết chat: gắn token lỗi: {e:?}");
            return vec![reply(chat_id, unknown.later)];
        }
    };
    let lang = reply_lang(lang, chat_lang);
    let words = texts(lang);
    let account = match store.account(&user_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return vec![reply(chat_id, words.guide)],
        Err(e) => {
            tracing::warn!("liên kết chat: đọc user lỗi: {e:?}");
            return vec![reply(chat_id, words.later)];
        }
    };
    if account.blocked {
        return vec![reply(chat_id, words.blocked)];
    }
    let text = confirm_question(lang, &mask_email(&account.email));
    let buttons = vec![
        (words.confirm_button.to_string(), format!("{CONFIRM_PREFIX}{token}")),
        (words.cancel_button.to_string(), format!("{CANCEL_PREFIX}{token}")),
    ];
    vec![Effect::Reply { chat_id, text, buttons }]
}

/// Một lần bấm nút trên tin hỏi xác nhận.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Press<'a> {
    pub callback_id: &'a str,
    pub chat_id: i64,
    /// Tin có nút (để gỡ nút).
    pub seq: Option<i64>,
    pub data: &'a str,
    /// Tên trên chat của người bấm (đã làm sạch), lưu cùng liên kết.
    pub chat_name: Option<&'a str>,
    /// Ngôn ngữ giao diện chat của người bấm.
    pub chat_lang: Option<Lang>,
}

/// Lần bấm nút trên tin hỏi xác nhận.
pub async fn on_press(store: &impl Store, press: Press<'_>) -> Vec<Effect> {
    let Press { callback_id, chat_id, seq, data, chat_name, chat_lang } = press;
    let clear = seq.map(|seq| Effect::ClearButtons { chat_id, seq });
    let unknown = texts(reply_lang(None, chat_lang));
    if let Some(token) = data.strip_prefix(CANCEL_PREFIX) {
        // Token đã hết hạn hay đã bị bỏ thì không còn ngôn ngữ lưu: theo ngôn ngữ chat.
        let lang = store.drop_token(token, chat_id).await.unwrap_or_else(|e| {
            tracing::warn!("liên kết chat: huỷ token lỗi: {e:?}");
            None
        });
        let words = texts(reply_lang(lang, chat_lang));
        let mut out = vec![answer(callback_id, words.pressed_cancel)];
        out.extend(clear);
        out.push(reply(chat_id, words.cancelled));
        return out;
    }
    let Some(token) = data.strip_prefix(CONFIRM_PREFIX) else {
        return vec![answer(callback_id, unknown.stale_button)];
    };
    let TokenOwner { user_id, lang } = match store.take_token(token, chat_id).await {
        Ok(Some(owner)) => owner,
        Ok(None) => {
            let mut out = vec![answer(callback_id, unknown.stale_button)];
            out.extend(clear);
            out.push(reply(chat_id, unknown.guide));
            return out;
        }
        // Token chưa bị dùng: bấm lại nút được.
        Err(e) => {
            tracing::warn!("liên kết chat: dùng token lỗi: {e:?}");
            return vec![answer(callback_id, unknown.later)];
        }
    };
    let words = texts(reply_lang(lang, chat_lang));
    // Từ đây token đã bị dùng: lỗi thì phải tạo đường dẫn mới.
    let failed = |text: &str| {
        let mut out = vec![answer(callback_id, words.pressed_failed)];
        out.extend(clear.clone());
        out.push(reply(chat_id, text));
        out
    };
    match store.account(&user_id).await {
        Ok(Some(a)) if a.blocked => return failed(words.blocked),
        Ok(Some(_)) => {}
        Ok(None) => return failed(words.guide),
        Err(e) => {
            tracing::warn!("liên kết chat: đọc user lỗi: {e:?}");
            return failed(words.retry_new);
        }
    }
    let previous = match store.link(&user_id, chat_id, chat_name).await {
        Ok(Some(previous)) => previous,
        Ok(None) => return failed(words.guide),
        Err(e) => {
            tracing::warn!("liên kết chat: ghi liên kết lỗi: {e:?}");
            return failed(words.retry_new);
        }
    };
    let mut out = vec![answer(callback_id, words.pressed_linked)];
    out.extend(clear);
    out.push(reply(chat_id, words.linked));
    out.push(Effect::Audit { user_id: user_id.clone(), action: "chat.link", details: doc! { "chatId": chat_id } });
    if let Some(old) = previous.filter(|old| *old != chat_id) {
        out.push(reply(old, words.moved));
    }
    out
}

/// `/unlink` trong chat: gỡ mọi tài khoản đang liên kết tới chat này. Lệnh không mang token nên bot trả lời theo ngôn
/// ngữ giao diện chat của người gửi.
pub async fn on_unlink(store: &impl Store, chat_id: i64, chat_lang: Option<Lang>) -> Vec<Effect> {
    let words = texts(reply_lang(None, chat_lang));
    match store.unlink_chat(chat_id).await {
        Ok(users) if users.is_empty() => vec![reply(chat_id, words.not_linked)],
        Ok(users) => {
            let mut out = vec![reply(chat_id, words.unlinked)];
            out.extend(users.into_iter().map(|user_id| Effect::Audit {
                user_id,
                action: "chat.unlink",
                details: doc! { "reason": "chat_command", "chatId": chat_id },
            }));
            out
        }
        Err(e) => {
            tracing::warn!("liên kết chat: gỡ theo /unlink lỗi: {e:?}");
            vec![reply(chat_id, words.later)]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Token: user, chat đã gắn. User: tài khoản, chat đang liên kết.
    #[derive(Default)]
    struct Fake {
        tokens: Mutex<HashMap<String, (String, Option<i64>)>>,
        users: Mutex<HashMap<String, (Account, Option<i64>)>>,
        names: Mutex<HashMap<String, Option<String>>>,
        fail: bool,
        /// Ngôn ngữ lưu trong mọi token giả.
        lang: Option<Lang>,
    }

    impl Fake {
        fn with(tokens: &[(&str, &str)], users: &[(&str, &str, bool, Option<i64>)]) -> Fake {
            let fake = Fake::default();
            for (t, u) in tokens {
                fake.tokens.lock().expect("khoá").insert(t.to_string(), (u.to_string(), None));
            }
            for (u, email, blocked, chat) in users {
                let account = Account { email: email.to_string(), blocked: *blocked };
                fake.users.lock().expect("khoá").insert(u.to_string(), (account, *chat));
            }
            fake
        }

        fn chat_of(&self, user: &str) -> Option<i64> {
            self.users.lock().expect("khoá").get(user).and_then(|(_, c)| *c)
        }

        fn err<T>(&self) -> Result<T, AppError> {
            Err(AppError::Internal("mongo giả lỗi".into()))
        }
    }

    impl Store for Fake {
        async fn bind_token(&self, token: &str, chat_id: i64) -> Result<Option<TokenOwner>, AppError> {
            if self.fail {
                return self.err();
            }
            let mut tokens = self.tokens.lock().expect("khoá");
            Ok(match tokens.get_mut(token) {
                Some((user, bound)) if bound.is_none_or(|c| c == chat_id) => {
                    *bound = Some(chat_id);
                    Some(TokenOwner { user_id: user.clone(), lang: self.lang })
                }
                _ => None,
            })
        }

        async fn take_token(&self, token: &str, chat_id: i64) -> Result<Option<TokenOwner>, AppError> {
            if self.fail {
                return self.err();
            }
            let mut tokens = self.tokens.lock().expect("khoá");
            if tokens.get(token).is_some_and(|(_, bound)| *bound == Some(chat_id)) {
                return Ok(tokens.remove(token).map(|(user_id, _)| TokenOwner { user_id, lang: self.lang }));
            }
            Ok(None)
        }

        async fn drop_token(&self, token: &str, chat_id: i64) -> Result<Option<Lang>, AppError> {
            let mut tokens = self.tokens.lock().expect("khoá");
            if tokens.get(token).is_some_and(|(_, bound)| *bound == Some(chat_id)) {
                tokens.remove(token);
                return Ok(self.lang);
            }
            Ok(None)
        }

        async fn account(&self, user_id: &str) -> Result<Option<Account>, AppError> {
            Ok(self.users.lock().expect("khoá").get(user_id).map(|(a, _)| a.clone()))
        }

        async fn link(&self, user_id: &str, chat_id: i64, chat_name: Option<&str>) -> Result<Option<Option<i64>>, AppError> {
            let mut users = self.users.lock().expect("khoá");
            let previous = users.get_mut(user_id).map(|(_, chat)| chat.replace(chat_id));
            if previous.is_some() {
                self.names.lock().expect("khoá").insert(user_id.to_string(), chat_name.map(str::to_string));
            }
            Ok(previous)
        }

        async fn unlink_chat(&self, chat_id: i64) -> Result<Vec<String>, AppError> {
            let mut users = self.users.lock().expect("khoá");
            let mut out = Vec::new();
            for (id, (_, chat)) in users.iter_mut() {
                if *chat == Some(chat_id) {
                    *chat = None;
                    out.push(id.clone());
                }
            }
            Ok(out)
        }
    }

    fn press<'a>(callback_id: &'a str, chat_id: i64, seq: Option<i64>, data: &'a str) -> Press<'a> {
        Press { callback_id, chat_id, seq, data, chat_name: None, chat_lang: None }
    }

    fn texts(effects: &[Effect]) -> Vec<(i64, &str)> {
        effects
            .iter()
            .filter_map(|e| match e {
                Effect::Reply { chat_id, text, .. } => Some((*chat_id, text.as_str())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn emails_are_masked() {
        assert_eq!(mask_email("an.nguyen@inet.vn"), "a***@inet.vn");
        assert_eq!(mask_email("đạt@x.vn"), "đ***@x.vn");
        assert_eq!(mask_email("@x.vn"), "***");
        assert_eq!(mask_email("khong-co"), "***");
    }

    #[tokio::test]
    async fn start_only_asks_and_does_not_link() {
        let store = Fake::with(&[("tok", "u1")], &[("u1", "an@inet.vn", false, None)]);
        let effects = on_start(&store, 12, Some("tok"), None).await;
        let [Effect::Reply { chat_id: 12, text, buttons }] = effects.as_slice() else { panic!("{effects:?}") };
        assert!(text.contains("a***@inet.vn"));
        assert_eq!(buttons, &vec![("Xác nhận liên kết".into(), "link:tok".into()), ("Huỷ".into(), "cancel:tok".into())]);
        assert_eq!(store.chat_of("u1"), None, "/start chưa được liên kết");
        // Chat khác dùng lại token đã gắn: không hỏi, không liên kết.
        assert_eq!(texts(&on_start(&store, 99, Some("tok"), None).await), vec![(99, VI.guide)]);
        assert_eq!(texts(&on_start(&store, 12, None, None).await), vec![(12, VI.guide)]);
    }

    #[tokio::test]
    async fn confirm_from_the_same_chat_links_and_mentions_unlink() {
        let store = Fake::with(&[("tok", "u1")], &[("u1", "an@inet.vn", false, Some(7))]);
        on_start(&store, 12, Some("tok"), None).await;
        // Bấm từ chat khác (không thể với nút thật, nhưng callback giả mạo): không liên kết.
        let other = on_press(&store, press("cb0", 99, Some(1), "link:tok")).await;
        assert_eq!(store.chat_of("u1"), Some(7));
        assert!(matches!(&other[0], Effect::Answer { .. }));

        let effects = on_press(&store, Press { chat_name: Some("An Nguyễn"), ..press("cb1", 12, Some(40), "link:tok") }).await;
        assert_eq!(store.chat_of("u1"), Some(12));
        assert_eq!(store.names.lock().expect("khoá").get("u1"), Some(&Some("An Nguyễn".to_string())), "lưu tên chat");
        assert_eq!(effects[0], Effect::Answer { callback_id: "cb1".into(), text: VI.pressed_linked.into() });
        assert_eq!(effects[1], Effect::ClearButtons { chat_id: 12, seq: 40 });
        assert_eq!(texts(&effects), vec![(12, VI.linked), (7, VI.moved)], "chat cũ được báo");
        assert!(VI.linked.contains("/unlink"));
        assert!(effects.iter().any(|e| matches!(e, Effect::Audit { action: "chat.link", .. })));
        // Token đã dùng: bấm lại không liên kết lần nữa.
        assert_eq!(texts(&on_press(&store, press("cb2", 12, None, "link:tok")).await), vec![(12, VI.guide)]);
    }

    #[tokio::test]
    async fn relinking_the_same_chat_does_not_notify_it() {
        let store = Fake::with(&[("tok", "u1")], &[("u1", "an@inet.vn", false, Some(12))]);
        on_start(&store, 12, Some("tok"), None).await;
        assert_eq!(texts(&on_press(&store, press("cb", 12, None, "link:tok")).await), vec![(12, VI.linked)]);
    }

    #[tokio::test]
    async fn cancel_drops_the_pending_token() {
        let store = Fake::with(&[("tok", "u1")], &[("u1", "an@inet.vn", false, None)]);
        on_start(&store, 12, Some("tok"), None).await;
        let effects = on_press(&store, press("cb", 12, Some(5), "cancel:tok")).await;
        assert_eq!(texts(&effects), vec![(12, VI.cancelled)]);
        assert!(effects.contains(&Effect::ClearButtons { chat_id: 12, seq: 5 }));
        on_press(&store, press("cb", 12, None, "link:tok")).await;
        assert_eq!(store.chat_of("u1"), None);
        assert_eq!(on_press(&store, press("cb", 12, None, "rác")).await, vec![answer("cb", VI.stale_button)]);
    }

    #[tokio::test]
    async fn blocked_accounts_are_not_linked() {
        let store = Fake::with(&[("tok", "u1")], &[("u1", "an@inet.vn", true, None)]);
        assert_eq!(texts(&on_start(&store, 12, Some("tok"), None).await), vec![(12, VI.blocked)]);
        assert_eq!(texts(&on_press(&store, press("cb", 12, None, "link:tok")).await), vec![(12, VI.blocked)]);
        assert_eq!(store.chat_of("u1"), None);
    }

    #[tokio::test]
    async fn failures_before_the_token_is_used_say_retry_later() {
        let store = Fake { fail: true, ..Fake::with(&[("tok", "u1")], &[("u1", "an@inet.vn", false, None)]) };
        assert_eq!(texts(&on_start(&store, 12, Some("tok"), None).await), vec![(12, VI.later)]);
        assert_eq!(on_press(&store, press("cb", 12, None, "link:tok")).await, vec![answer("cb", VI.later)]);
    }

    #[tokio::test]
    async fn failures_after_the_token_is_used_ask_for_a_new_link() {
        /// Store dùng token được nhưng ghi liên kết lỗi.
        struct LinkFails(Fake);
        impl Store for LinkFails {
            async fn bind_token(&self, t: &str, c: i64) -> Result<Option<TokenOwner>, AppError> {
                self.0.bind_token(t, c).await
            }
            async fn take_token(&self, t: &str, c: i64) -> Result<Option<TokenOwner>, AppError> {
                self.0.take_token(t, c).await
            }
            async fn drop_token(&self, t: &str, c: i64) -> Result<Option<Lang>, AppError> {
                self.0.drop_token(t, c).await
            }
            async fn account(&self, u: &str) -> Result<Option<Account>, AppError> {
                self.0.account(u).await
            }
            async fn link(&self, _: &str, _: i64, _: Option<&str>) -> Result<Option<Option<i64>>, AppError> {
                Err(AppError::Internal("ghi lỗi".into()))
            }
            async fn unlink_chat(&self, c: i64) -> Result<Vec<String>, AppError> {
                self.0.unlink_chat(c).await
            }
        }
        let store = LinkFails(Fake::with(&[("tok", "u1")], &[("u1", "an@inet.vn", false, None)]));
        on_start(&store, 12, Some("tok"), None).await;
        let effects = on_press(&store, press("cb", 12, None, "link:tok")).await;
        assert_eq!(texts(&effects), vec![(12, VI.retry_new)]);
        assert!(VI.retry_new.contains("Tạo đường dẫn mới trong Scribe"));
    }

    #[tokio::test]
    async fn unlink_command_removes_every_account_of_the_chat() {
        let store = Fake::with(&[], &[("u1", "a@x.vn", false, Some(12)), ("u2", "b@x.vn", false, Some(13))]);
        let effects = on_unlink(&store, 12, None).await;
        assert_eq!(texts(&effects), vec![(12, VI.unlinked)]);
        assert!(effects.iter().any(|e| matches!(e, Effect::Audit { action: "chat.unlink", user_id, .. } if user_id == "u1")));
        assert_eq!((store.chat_of("u1"), store.chat_of("u2")), (None, Some(13)));
        assert_eq!(texts(&on_unlink(&store, 12, None).await), vec![(12, VI.not_linked)]);
    }

    #[tokio::test]
    async fn a_chinese_link_is_answered_in_chinese() {
        let store = Fake { lang: Some(Lang::Zh), ..Fake::with(&[("tok", "u1"), ("t2", "u1")], &[("u1", "an@inet.vn", false, Some(7))]) };
        let effects = on_start(&store, 12, Some("tok"), None).await;
        let [Effect::Reply { text, buttons, .. }] = effects.as_slice() else { panic!("{effects:?}") };
        assert!(text.starts_with("将这个聊天关联到 Scribe 账号 a***@inet.vn？") && text.contains("“确认关联”"), "{text}");
        assert_eq!(buttons, &vec![("确认关联".into(), "link:tok".into()), ("取消".into(), "cancel:tok".into())]);
        let effects = on_press(&store, press("cb", 12, None, "link:tok")).await;
        assert_eq!(effects[0], answer("cb", ZH.pressed_linked));
        assert_eq!(texts(&effects), vec![(12, ZH.linked), (7, ZH.moved)]);
        on_start(&store, 12, Some("t2"), None).await;
        let effects = on_press(&store, press("cb", 12, None, "cancel:t2")).await;
        assert_eq!(effects[0], answer("cb", ZH.pressed_cancel));
        assert_eq!(texts(&effects), vec![(12, ZH.cancelled)]);
        // Không còn token để biết ngôn ngữ, chat không cho biết ngôn ngữ giao diện: tiếng Việt.
        assert_eq!(texts(&on_press(&store, press("cb", 12, None, "link:tok")).await), vec![(12, VI.guide)]);
        assert_eq!(texts(&on_unlink(&store, 12, None).await), vec![(12, VI.unlinked)]);
    }

    #[tokio::test]
    async fn without_a_known_user_the_bot_answers_in_the_chat_language() {
        let store = Fake::with(&[], &[("u1", "a@x.vn", false, Some(12))]);
        let en = Some(Lang::En);
        assert_eq!(texts(&on_start(&store, 12, None, en).await), vec![(12, EN.guide)]);
        assert_eq!(texts(&on_start(&store, 12, Some("het-han"), Some(Lang::Zh)).await), vec![(12, ZH.guide)]);
        let stale = on_press(&store, Press { chat_lang: en, ..press("cb", 12, Some(4), "link:het-han") }).await;
        assert_eq!(stale[0], answer("cb", EN.stale_button));
        assert_eq!(texts(&stale), vec![(12, EN.guide)]);
        assert_eq!(on_press(&store, Press { chat_lang: en, ..press("cb", 12, None, "rác") }).await, vec![answer("cb", EN.stale_button)]);
        let cancel = on_press(&store, Press { chat_lang: en, ..press("cb", 12, None, "cancel:het-han") }).await;
        assert_eq!(cancel[0], answer("cb", EN.pressed_cancel));
        assert_eq!(texts(&on_unlink(&store, 12, Some(Lang::Zh)).await), vec![(12, ZH.unlinked)]);
        assert_eq!(texts(&on_unlink(&store, 12, en).await), vec![(12, EN.not_linked)]);
        let failing = Fake { fail: true, ..Fake::default() };
        assert_eq!(texts(&on_start(&failing, 12, Some("tok"), en).await), vec![(12, EN.later)]);
        assert_eq!(on_press(&failing, Press { chat_lang: en, ..press("cb", 12, None, "link:tok") }).await, vec![answer("cb", EN.later)]);
    }

    #[tokio::test]
    async fn the_language_saved_with_the_link_wins_over_the_chat_language() {
        let store = Fake { lang: Some(Lang::Zh), ..Fake::with(&[("tok", "u1")], &[("u1", "an@inet.vn", false, None)]) };
        let effects = on_start(&store, 12, Some("tok"), Some(Lang::En)).await;
        let [Effect::Reply { buttons, .. }] = effects.as_slice() else { panic!("{effects:?}") };
        assert_eq!(buttons[0].0, ZH.confirm_button);
        let effects = on_press(&store, Press { chat_lang: Some(Lang::En), ..press("cb", 12, None, "link:tok") }).await;
        assert_eq!(texts(&effects), vec![(12, ZH.linked)]);
    }

    #[tokio::test]
    async fn a_link_without_a_saved_language_follows_the_chat_language() {
        let store = Fake::with(&[("tok", "u1"), ("t2", "u1")], &[("u1", "an@inet.vn", false, None)]);
        let effects = on_start(&store, 12, Some("tok"), Some(Lang::En)).await;
        let [Effect::Reply { text, buttons, .. }] = effects.as_slice() else { panic!("{effects:?}") };
        assert!(text.starts_with("Link this chat"), "{text}");
        assert_eq!(buttons[0].0, EN.confirm_button);
        let effects = on_press(&store, Press { chat_lang: Some(Lang::En), ..press("cb", 12, None, "link:tok") }).await;
        assert_eq!(texts(&effects), vec![(12, EN.linked)]);
        on_start(&store, 12, Some("t2"), None).await;
        let effects = on_press(&store, Press { chat_lang: Some(Lang::Zh), ..press("cb", 12, None, "cancel:t2") }).await;
        assert_eq!(texts(&effects), vec![(12, ZH.cancelled)]);
    }

    #[test]
    fn reply_language_prefers_the_saved_one_then_the_chat_then_vietnamese() {
        assert_eq!(reply_lang(Some(Lang::Zh), Some(Lang::En)), Lang::Zh);
        assert_eq!(reply_lang(None, Some(Lang::En)), Lang::En);
        assert_eq!(reply_lang(None, None), Lang::Vi);
    }

    #[test]
    fn every_language_names_the_same_confirm_button_in_its_question() {
        for lang in Lang::ALL {
            let question = confirm_question(lang, "a***@x.vn");
            assert!(question.contains(super::texts(lang).confirm_button) && question.contains("a***@x.vn"), "{question}");
            assert!(super::texts(lang).linked.contains("/unlink"));
        }
    }
}
