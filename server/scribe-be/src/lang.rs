//! Ngôn ngữ của chữ server viết cho người dùng: đích của bản dịch, và ngôn ngữ giao diện web gửi kèm request
//! (`lang`) để biên bản, câu trả lời hỏi đáp và tin của bot viết đúng ngôn ngữ người đó đang dùng. Ba ngôn ngữ
//! khớp `LOCALES` của web (`web/src/i18n/locales.ts`): `vi` (mặc định), `en`, `zh-CN` (tiếng Trung giản thể).

use crate::error::AppError;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Lang {
    En,
    #[default]
    Vi,
    /// Tiếng Trung giản thể, mã `zh-CN` như web.
    Zh,
}

impl Lang {
    /// Thứ tự liệt kê (trạng thái bản dịch của phiên).
    pub const ALL: [Lang; 3] = [Lang::En, Lang::Vi, Lang::Zh];

    pub fn parse(raw: &str) -> Option<Lang> {
        match raw {
            "en" => Some(Lang::En),
            "vi" => Some(Lang::Vi),
            "zh-CN" => Some(Lang::Zh),
            _ => None,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Vi => "vi",
            Lang::Zh => "zh-CN",
        }
    }

    /// Tên tiếng Anh, dùng trong chỉ dẫn gửi model.
    pub fn english_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Vi => "Vietnamese",
            Lang::Zh => "Simplified Chinese",
        }
    }
}

/// Câu lỗi khi `lang` không phải mã đã biết.
pub const UNKNOWN: &str = "ngôn ngữ chỉ nhận en, vi hoặc zh-CN";

/// Ngôn ngữ `lang` của một request: thiếu hoặc rỗng ⟹ tiếng Việt (client cũ, app iPhone bản trước không gửi),
/// mã lạ ⟹ 400.
pub fn from_request(raw: Option<&str>) -> Result<Lang, AppError> {
    match raw.filter(|r| !r.is_empty()) {
        None => Ok(Lang::Vi),
        Some(raw) => Lang::parse(raw).ok_or_else(|| AppError::BadRequest(UNKNOWN.into())),
    }
}

/// Body `{lang?}` của các request chỉ cần biết ngôn ngữ người dùng (tóm tắt, tạo đường dẫn liên kết chat). Client
/// cũ gửi không body: handler nhận `Option<Json<LangBody>>`.
#[derive(Debug, Default, Deserialize)]
pub struct LangBody {
    pub lang: Option<String>,
}

impl LangBody {
    /// Ngôn ngữ của body (thiếu body ⟹ tiếng Việt), theo luật của [`from_request`].
    pub fn lang_of(body: Option<&LangBody>) -> Result<Lang, AppError> {
        from_request(body.and_then(|b| b.lang.as_deref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_match_the_web_locales() {
        for lang in Lang::ALL {
            assert_eq!(Lang::parse(lang.code()), Some(lang));
        }
        assert_eq!(Lang::ALL.map(Lang::code), ["en", "vi", "zh-CN"]);
        assert_eq!(Lang::parse("zh"), None);
        assert_eq!(Lang::parse("EN"), None);
    }

    #[test]
    fn requests_default_to_vietnamese_and_refuse_unknown_codes() {
        assert_eq!(from_request(None).expect("thiếu"), Lang::Vi);
        assert_eq!(from_request(Some("")).expect("rỗng"), Lang::Vi);
        assert_eq!(from_request(Some("zh-CN")).expect("zh"), Lang::Zh);
        assert_eq!(from_request(Some("en")).expect("en"), Lang::En);
        assert!(matches!(from_request(Some("fr")), Err(AppError::BadRequest(_))));
        assert_eq!(LangBody::lang_of(None).expect("không body"), Lang::Vi);
        let body: LangBody = serde_json::from_str(r#"{"lang":"zh-CN"}"#).expect("json");
        assert_eq!(LangBody::lang_of(Some(&body)).expect("zh"), Lang::Zh);
        assert_eq!(LangBody::lang_of(Some(&LangBody::default())).expect("{}"), Lang::Vi);
    }
}
