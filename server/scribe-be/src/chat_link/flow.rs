//! Hội thoại liên kết trong chat riêng với bot, tách khỏi Mongo và Bot API để kiểm được bằng store giả.
//!
//! Chống liên kết hộ (kẻ xấu gửi đường dẫn của tài khoản mình cho người khác bấm, để biên bản của kẻ xấu
//! chảy vào chat nạn nhân hoặc ngược lại): `/start <token>` chưa liên kết gì, bot chỉ gắn token với chat
//! đó và hỏi lại, nêu tài khoản Scribe (email che bớt) kèm nút "Xác nhận liên kết" / "Huỷ". Chỉ lần bấm
//! "Xác nhận" từ đúng chat đó mới dùng token và liên kết. Liên kết xong bot nhắc `/unlink` để gỡ; liên
//! kết sang chat khác thì chat cũ được báo.

use crate::error::AppError;
use mongodb::bson::{doc, Document};
use std::future::Future;

pub const LINKED: &str =
    "Đã liên kết với Scribe. Biên bản bạn gửi từ Scribe sẽ tới chat này. Gửi /unlink nếu muốn gỡ liên kết.";
pub const GUIDE: &str = "Liên kết không hợp lệ hoặc đã hết hạn. Mở Scribe, chọn \"Liên kết chat\" rồi bấm lại đường dẫn mới (đường dẫn dùng được 10 phút).";
pub const BLOCKED: &str = "Tài khoản Scribe này đang bị khoá nên không liên kết được.";
/// Token chưa bị dùng: thử lại chính đường dẫn đó được.
pub const LATER: &str = "Scribe đang bận, thử lại sau ít phút.";
/// Token đã bị dùng ở bước xác nhận: đường dẫn cũ không dùng lại được nữa.
pub const RETRY_NEW: &str = "Không liên kết được lúc này. Tạo đường dẫn mới trong Scribe rồi thử lại.";
pub const CANCELLED: &str = "Đã huỷ, chat này không được liên kết.";
pub const UNLINKED: &str = "Đã gỡ liên kết. Scribe sẽ không gửi biên bản tới chat này nữa.";
pub const NOT_LINKED: &str = "Chat này chưa liên kết với tài khoản Scribe nào.";
pub const MOVED: &str = "Tài khoản Scribe của bạn vừa được liên kết với một chat khác; chat này không nhận biên bản nữa.";
const STALE_BUTTON: &str = "Nút này không còn dùng được.";

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
    /// Gắn token còn hạn với chat (chat đầu tiên gửi nó); trả user của token.
    fn bind_token(&self, token: &str, chat_id: i64) -> impl Future<Output = Result<Option<String>, AppError>> + Send;
    /// Dùng token đã gắn với đúng chat này (một lần); trả user nếu còn hạn.
    fn take_token(&self, token: &str, chat_id: i64) -> impl Future<Output = Result<Option<String>, AppError>> + Send;
    fn drop_token(&self, token: &str, chat_id: i64) -> impl Future<Output = Result<bool, AppError>> + Send;
    fn account(&self, user_id: &str) -> impl Future<Output = Result<Option<Account>, AppError>> + Send;
    /// `None` khi không còn user; `Some(chat cũ)` khi đã gắn.
    fn link(&self, user_id: &str, chat_id: i64) -> impl Future<Output = Result<Option<Option<i64>>, AppError>> + Send;
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

/// `/start <token>`: hỏi lại trước khi liên kết.
pub async fn on_start(store: &impl Store, chat_id: i64, token: Option<&str>) -> Vec<Effect> {
    let Some(token) = token else { return vec![reply(chat_id, GUIDE)] };
    let user_id = match store.bind_token(token, chat_id).await {
        Ok(Some(u)) => u,
        Ok(None) => return vec![reply(chat_id, GUIDE)],
        Err(e) => {
            tracing::warn!("liên kết chat: gắn token lỗi: {e:?}");
            return vec![reply(chat_id, LATER)];
        }
    };
    let account = match store.account(&user_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return vec![reply(chat_id, GUIDE)],
        Err(e) => {
            tracing::warn!("liên kết chat: đọc user lỗi: {e:?}");
            return vec![reply(chat_id, LATER)];
        }
    };
    if account.blocked {
        return vec![reply(chat_id, BLOCKED)];
    }
    let text = format!(
        "Liên kết chat này với tài khoản Scribe {}?\nChỉ bấm \"Xác nhận liên kết\" nếu đây là tài khoản của bạn và bạn vừa mở đường dẫn từ Scribe.",
        mask_email(&account.email)
    );
    let buttons = vec![
        ("Xác nhận liên kết".to_string(), format!("{CONFIRM_PREFIX}{token}")),
        ("Huỷ".to_string(), format!("{CANCEL_PREFIX}{token}")),
    ];
    vec![Effect::Reply { chat_id, text, buttons }]
}

/// Lần bấm nút trên tin hỏi xác nhận. `seq` là tin có nút (để gỡ nút).
pub async fn on_press(store: &impl Store, callback_id: &str, chat_id: i64, seq: Option<i64>, data: &str) -> Vec<Effect> {
    let clear = seq.map(|seq| Effect::ClearButtons { chat_id, seq });
    if let Some(token) = data.strip_prefix(CANCEL_PREFIX) {
        if let Err(e) = store.drop_token(token, chat_id).await {
            tracing::warn!("liên kết chat: huỷ token lỗi: {e:?}");
        }
        let mut out = vec![answer(callback_id, "Đã huỷ.")];
        out.extend(clear);
        out.push(reply(chat_id, CANCELLED));
        return out;
    }
    let Some(token) = data.strip_prefix(CONFIRM_PREFIX) else {
        return vec![answer(callback_id, STALE_BUTTON)];
    };
    let user_id = match store.take_token(token, chat_id).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            let mut out = vec![answer(callback_id, STALE_BUTTON)];
            out.extend(clear);
            out.push(reply(chat_id, GUIDE));
            return out;
        }
        // Token chưa bị dùng: bấm lại nút được.
        Err(e) => {
            tracing::warn!("liên kết chat: dùng token lỗi: {e:?}");
            return vec![answer(callback_id, LATER)];
        }
    };
    // Từ đây token đã bị dùng: lỗi thì phải tạo đường dẫn mới.
    let failed = |text: &str| {
        let mut out = vec![answer(callback_id, "Không liên kết được.")];
        out.extend(clear.clone());
        out.push(reply(chat_id, text));
        out
    };
    match store.account(&user_id).await {
        Ok(Some(a)) if a.blocked => return failed(BLOCKED),
        Ok(Some(_)) => {}
        Ok(None) => return failed(GUIDE),
        Err(e) => {
            tracing::warn!("liên kết chat: đọc user lỗi: {e:?}");
            return failed(RETRY_NEW);
        }
    }
    let previous = match store.link(&user_id, chat_id).await {
        Ok(Some(previous)) => previous,
        Ok(None) => return failed(GUIDE),
        Err(e) => {
            tracing::warn!("liên kết chat: ghi liên kết lỗi: {e:?}");
            return failed(RETRY_NEW);
        }
    };
    let mut out = vec![answer(callback_id, "Đã liên kết.")];
    out.extend(clear);
    out.push(reply(chat_id, LINKED));
    out.push(Effect::Audit { user_id: user_id.clone(), action: "chat.link", details: doc! { "chatId": chat_id } });
    if let Some(old) = previous.filter(|old| *old != chat_id) {
        out.push(reply(old, MOVED));
    }
    out
}

/// `/unlink` trong chat: gỡ mọi tài khoản đang liên kết tới chat này.
pub async fn on_unlink(store: &impl Store, chat_id: i64) -> Vec<Effect> {
    match store.unlink_chat(chat_id).await {
        Ok(users) if users.is_empty() => vec![reply(chat_id, NOT_LINKED)],
        Ok(users) => {
            let mut out = vec![reply(chat_id, UNLINKED)];
            out.extend(users.into_iter().map(|user_id| Effect::Audit {
                user_id,
                action: "chat.unlink",
                details: doc! { "reason": "chat_command", "chatId": chat_id },
            }));
            out
        }
        Err(e) => {
            tracing::warn!("liên kết chat: gỡ theo /unlink lỗi: {e:?}");
            vec![reply(chat_id, LATER)]
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
        fail: bool,
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
        async fn bind_token(&self, token: &str, chat_id: i64) -> Result<Option<String>, AppError> {
            if self.fail {
                return self.err();
            }
            let mut tokens = self.tokens.lock().expect("khoá");
            Ok(match tokens.get_mut(token) {
                Some((user, bound)) if bound.is_none_or(|c| c == chat_id) => {
                    *bound = Some(chat_id);
                    Some(user.clone())
                }
                _ => None,
            })
        }

        async fn take_token(&self, token: &str, chat_id: i64) -> Result<Option<String>, AppError> {
            if self.fail {
                return self.err();
            }
            let mut tokens = self.tokens.lock().expect("khoá");
            if tokens.get(token).is_some_and(|(_, bound)| *bound == Some(chat_id)) {
                return Ok(tokens.remove(token).map(|(u, _)| u));
            }
            Ok(None)
        }

        async fn drop_token(&self, token: &str, chat_id: i64) -> Result<bool, AppError> {
            let mut tokens = self.tokens.lock().expect("khoá");
            if tokens.get(token).is_some_and(|(_, bound)| *bound == Some(chat_id)) {
                tokens.remove(token);
                return Ok(true);
            }
            Ok(false)
        }

        async fn account(&self, user_id: &str) -> Result<Option<Account>, AppError> {
            Ok(self.users.lock().expect("khoá").get(user_id).map(|(a, _)| a.clone()))
        }

        async fn link(&self, user_id: &str, chat_id: i64) -> Result<Option<Option<i64>>, AppError> {
            let mut users = self.users.lock().expect("khoá");
            Ok(users.get_mut(user_id).map(|(_, chat)| chat.replace(chat_id)))
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
        let effects = on_start(&store, 12, Some("tok")).await;
        let [Effect::Reply { chat_id: 12, text, buttons }] = effects.as_slice() else { panic!("{effects:?}") };
        assert!(text.contains("a***@inet.vn"));
        assert_eq!(buttons, &vec![("Xác nhận liên kết".into(), "link:tok".into()), ("Huỷ".into(), "cancel:tok".into())]);
        assert_eq!(store.chat_of("u1"), None, "/start chưa được liên kết");
        // Chat khác dùng lại token đã gắn: không hỏi, không liên kết.
        assert_eq!(texts(&on_start(&store, 99, Some("tok")).await), vec![(99, GUIDE)]);
        assert_eq!(texts(&on_start(&store, 12, None).await), vec![(12, GUIDE)]);
    }

    #[tokio::test]
    async fn confirm_from_the_same_chat_links_and_mentions_unlink() {
        let store = Fake::with(&[("tok", "u1")], &[("u1", "an@inet.vn", false, Some(7))]);
        on_start(&store, 12, Some("tok")).await;
        // Bấm từ chat khác (không thể với nút thật, nhưng callback giả mạo): không liên kết.
        let other = on_press(&store, "cb0", 99, Some(1), "link:tok").await;
        assert_eq!(store.chat_of("u1"), Some(7));
        assert!(matches!(&other[0], Effect::Answer { .. }));

        let effects = on_press(&store, "cb1", 12, Some(40), "link:tok").await;
        assert_eq!(store.chat_of("u1"), Some(12));
        assert_eq!(effects[0], Effect::Answer { callback_id: "cb1".into(), text: "Đã liên kết.".into() });
        assert_eq!(effects[1], Effect::ClearButtons { chat_id: 12, seq: 40 });
        assert_eq!(texts(&effects), vec![(12, LINKED), (7, MOVED)], "chat cũ được báo");
        assert!(LINKED.contains("/unlink"));
        assert!(effects.iter().any(|e| matches!(e, Effect::Audit { action: "chat.link", .. })));
        // Token đã dùng: bấm lại không liên kết lần nữa.
        assert_eq!(texts(&on_press(&store, "cb2", 12, None, "link:tok").await), vec![(12, GUIDE)]);
    }

    #[tokio::test]
    async fn relinking_the_same_chat_does_not_notify_it() {
        let store = Fake::with(&[("tok", "u1")], &[("u1", "an@inet.vn", false, Some(12))]);
        on_start(&store, 12, Some("tok")).await;
        assert_eq!(texts(&on_press(&store, "cb", 12, None, "link:tok").await), vec![(12, LINKED)]);
    }

    #[tokio::test]
    async fn cancel_drops_the_pending_token() {
        let store = Fake::with(&[("tok", "u1")], &[("u1", "an@inet.vn", false, None)]);
        on_start(&store, 12, Some("tok")).await;
        let effects = on_press(&store, "cb", 12, Some(5), "cancel:tok").await;
        assert_eq!(texts(&effects), vec![(12, CANCELLED)]);
        assert!(effects.contains(&Effect::ClearButtons { chat_id: 12, seq: 5 }));
        on_press(&store, "cb", 12, None, "link:tok").await;
        assert_eq!(store.chat_of("u1"), None);
        assert_eq!(on_press(&store, "cb", 12, None, "rác").await, vec![answer("cb", STALE_BUTTON)]);
    }

    #[tokio::test]
    async fn blocked_accounts_are_not_linked() {
        let store = Fake::with(&[("tok", "u1")], &[("u1", "an@inet.vn", true, None)]);
        assert_eq!(texts(&on_start(&store, 12, Some("tok")).await), vec![(12, BLOCKED)]);
        assert_eq!(texts(&on_press(&store, "cb", 12, None, "link:tok").await), vec![(12, BLOCKED)]);
        assert_eq!(store.chat_of("u1"), None);
    }

    #[tokio::test]
    async fn failures_before_the_token_is_used_say_retry_later() {
        let store = Fake { fail: true, ..Fake::with(&[("tok", "u1")], &[("u1", "an@inet.vn", false, None)]) };
        assert_eq!(texts(&on_start(&store, 12, Some("tok")).await), vec![(12, LATER)]);
        assert_eq!(on_press(&store, "cb", 12, None, "link:tok").await, vec![answer("cb", LATER)]);
    }

    #[tokio::test]
    async fn failures_after_the_token_is_used_ask_for_a_new_link() {
        /// Store dùng token được nhưng ghi liên kết lỗi.
        struct LinkFails(Fake);
        impl Store for LinkFails {
            async fn bind_token(&self, t: &str, c: i64) -> Result<Option<String>, AppError> {
                self.0.bind_token(t, c).await
            }
            async fn take_token(&self, t: &str, c: i64) -> Result<Option<String>, AppError> {
                self.0.take_token(t, c).await
            }
            async fn drop_token(&self, t: &str, c: i64) -> Result<bool, AppError> {
                self.0.drop_token(t, c).await
            }
            async fn account(&self, u: &str) -> Result<Option<Account>, AppError> {
                self.0.account(u).await
            }
            async fn link(&self, _: &str, _: i64) -> Result<Option<Option<i64>>, AppError> {
                Err(AppError::Internal("ghi lỗi".into()))
            }
            async fn unlink_chat(&self, c: i64) -> Result<Vec<String>, AppError> {
                self.0.unlink_chat(c).await
            }
        }
        let store = LinkFails(Fake::with(&[("tok", "u1")], &[("u1", "an@inet.vn", false, None)]));
        on_start(&store, 12, Some("tok")).await;
        let effects = on_press(&store, "cb", 12, None, "link:tok").await;
        assert_eq!(texts(&effects), vec![(12, RETRY_NEW)]);
        assert!(RETRY_NEW.contains("Tạo đường dẫn mới trong Scribe"));
    }

    #[tokio::test]
    async fn unlink_command_removes_every_account_of_the_chat() {
        let store = Fake::with(&[], &[("u1", "a@x.vn", false, Some(12)), ("u2", "b@x.vn", false, Some(13))]);
        let effects = on_unlink(&store, 12).await;
        assert_eq!(texts(&effects), vec![(12, UNLINKED)]);
        assert!(effects.iter().any(|e| matches!(e, Effect::Audit { action: "chat.unlink", user_id, .. } if user_id == "u1")));
        assert_eq!((store.chat_of("u1"), store.chat_of("u2")), (None, Some(13)));
        assert_eq!(texts(&on_unlink(&store, 12).await), vec![(12, NOT_LINKED)]);
    }
}
