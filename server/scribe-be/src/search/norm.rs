//! Chuẩn hoá chữ cho index tìm kiếm: gõ có dấu hay không dấu đều khớp.
//!
//! [`normalize`]: tách dấu (NFD) rồi bỏ dấu kết hợp, `đ`/`Đ` thành `d`, viết thường; mọi ký tự không phải
//! chữ hay số thành khoảng trắng; gộp khoảng trắng. Câu trong index và câu người dùng gõ đi cùng một
//! đường nên so được với nhau. Web làm đúng phép này từng ký tự để tô phần khớp
//! (`web/src/features/search/normalize.ts`).

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// Chữ đã chuẩn hoá: chỉ gồm token `[a-z0-9…]` cách nhau đúng một khoảng trắng.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = true;
    for c in text.nfd() {
        if is_combining_mark(c) {
            continue;
        }
        let c = match c {
            'đ' | 'Đ' => 'd',
            c => c,
        };
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
            space = false;
        } else if !space {
            out.push(' ');
            space = true;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

/// Token của chữ đã chuẩn hoá.
pub fn tokens(normalized: &str) -> Vec<&str> {
    normalized.split(' ').filter(|t| !t.is_empty()).collect()
}

/// Từ đệm tiếng Việt (đã bỏ dấu) không mang nghĩa khi tìm đoạn liên quan cho câu hỏi. Bỏ dấu làm vài từ
/// có nghĩa trùng dạng với từ đệm (vd "cổ" và "có" cùng là `co`); chấp nhận được vì câu hỏi thường còn
/// các từ khác.
const STOPWORDS: &[&str] = &[
    "a", "ah", "ai", "anh", "ba", "bao", "bay", "bi", "boi", "cac", "cai", "can", "cho", "chi", "chu", "chua",
    "co", "con", "cua", "cung", "da", "dang", "day", "de", "den", "deu", "di", "do", "du", "duoc", "gi", "giup",
    "ha", "hay", "hoac", "hoi", "khi", "khong", "la", "lai", "lam", "luc", "ma", "minh", "mot", "nao", "nay",
    "neu", "nhe", "nhi", "nhieu", "nhu", "nhung", "noi", "o", "oi", "ra", "rang", "roi", "sao", "se", "tai",
    "the", "thi", "thoi", "toi", "trong", "tu", "va", "vay", "ve", "vi", "voi", "vua", "xem", "ban", "em",
];

/// Từ khoá của câu hỏi để tìm đoạn liên quan: token đã chuẩn hoá, bỏ từ đệm, không trùng, giữ thứ tự.
pub fn keywords(question: &str) -> Vec<String> {
    let normalized = normalize(question);
    let mut out: Vec<String> = Vec::new();
    for t in tokens(&normalized) {
        if STOPWORDS.contains(&t) || out.iter().any(|o| o == t) {
            continue;
        }
        out.push(t.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_vietnamese_marks_and_folds_case() {
        assert_eq!(normalize("Hợp đồng"), "hop dong");
        assert_eq!(normalize("ĐỒNG Ý ký HỢP ĐỒNG!"), "dong y ky hop dong");
        assert_eq!(normalize("  Thứ   Sáu,\n15/10 — chốt giá 5.000đ "), "thu sau 15 10 chot gia 5 000d");
        assert_eq!(normalize("Nghiêng ngả ưởng ữ ỹ"), "nghieng nga uong u y");
        // Chữ đã tách dấu (NFD) cho cùng kết quả với chữ dựng sẵn.
        assert_eq!(normalize("ho\u{0323}\u{0302}i"), normalize("hội"));
        assert_eq!(normalize("hội"), "hoi");
        assert_eq!(normalize("Café naïve"), "cafe naive");
        assert_eq!(normalize("…!?"), "");
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn tokens_and_keywords() {
        assert_eq!(tokens("hop dong thue"), vec!["hop", "dong", "thue"]);
        assert!(tokens("").is_empty());
        assert_eq!(keywords("Họ đã chốt giá hợp đồng là bao nhiêu vậy?"), vec!["ho", "chot", "gia", "hop", "dong"]);
        assert_eq!(keywords("Có gì không?"), Vec::<String>::new());
        assert_eq!(keywords("giá giá GIÁ"), vec!["gia"]);
    }
}
