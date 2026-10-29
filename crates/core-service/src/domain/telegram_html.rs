//! Проверка текста поста на совместимость с HTML-разметкой Telegram Bot API
//! (https://core.telegram.org/bots/api#html-style). Нужна, чтобы ошибки разметки
//! всплывали при создании поста (Claude сразу их исправит), а не в 7 утра при
//! публикации.

/// Лимиты Telegram на длину видимого текста (после разбора разметки).
pub const MESSAGE_LIMIT: usize = 4096;
pub const CAPTION_LIMIT: usize = 1024;

const ALLOWED_TAGS: &[&str] = &[
    "b", "strong", "i", "em", "u", "ins", "s", "strike", "del", "a", "code", "pre", "blockquote", "tg-spoiler",
    "span", "tg-emoji",
];

/// Видимая длина: без тегов, с раскрытыми HTML-сущностями. Telegram ограничивает
/// именно её, а не длину исходной HTML-строки.
pub fn visible_len(html: &str) -> usize {
    to_plain_text(html).chars().count()
}

/// Текст без тегов с раскрытыми базовыми сущностями.
pub fn to_plain_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    decode_entities(&out)
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(pos) = rest.find('&') {
        out.push_str(&rest[..pos]);
        let tail = &rest[pos..];
        let decoded = tail.find(';').filter(|&end| end <= 10).and_then(|end| {
            let entity = &tail[1..end];
            let ch = match entity {
                "lt" => Some('<'),
                "gt" => Some('>'),
                "amp" => Some('&'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                e if e.starts_with("#x") || e.starts_with("#X") => {
                    u32::from_str_radix(&e[2..], 16).ok().and_then(char::from_u32)
                }
                e if e.starts_with('#') => e[1..].parse::<u32>().ok().and_then(char::from_u32),
                _ => None,
            };
            ch.map(|c| (c, end))
        });
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &tail[end + 1..];
            }
            None => {
                out.push('&');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Возвращает список проблем разметки (пустой — всё хорошо): неподдерживаемые
/// теги, незакрытые/лишние закрывающие теги, неэкранированные `<` и `&`.
pub fn validate(html: &str) -> Vec<String> {
    let mut problems: Vec<String> = Vec::new();
    let mut unsupported: Vec<String> = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let bytes = html.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'<' => {
                let Some(end_rel) = html[i..].find('>') else {
                    problems.push(format!("неэкранированный '<' в «{}» — пишите &lt;", snippet(&html[i..])));
                    break;
                };
                let inner = &html[i + 1..i + end_rel];
                let closing = inner.starts_with('/');
                let name: String = inner
                    .trim_start_matches('/')
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                    .collect::<String>()
                    .to_lowercase();
                if name.is_empty() {
                    problems.push(format!("неэкранированный '<' в «{}» — пишите &lt;", snippet(&html[i..])));
                } else if !ALLOWED_TAGS.contains(&name.as_str()) {
                    let tag = format!("<{name}>");
                    if !unsupported.contains(&tag) {
                        unsupported.push(tag);
                    }
                } else if closing {
                    match stack.pop() {
                        Some(open) if open == name => {}
                        Some(open) => problems.push(format!("тег </{name}> закрывает незакрытый <{open}>")),
                        None => problems.push(format!("лишний закрывающий тег </{name}>")),
                    }
                } else if !inner.ends_with('/') {
                    stack.push(name);
                }
                i += end_rel + 1;
            }
            b'&' => {
                let tail = &html[i..];
                let is_entity = tail
                    .find(';')
                    .filter(|&end| end > 1 && end <= 10)
                    .map(|end| tail[1..end].chars().all(|c| c.is_ascii_alphanumeric() || c == '#'))
                    .unwrap_or(false);
                if !is_entity {
                    problems.push(format!("неэкранированный '&' в «{}» — пишите &amp;", snippet(tail)));
                }
                i += 1;
            }
            b'>' => {
                problems.push(format!("неэкранированный '>' в «{}» — пишите &gt;", snippet(&html[i..])));
                i += 1;
            }
            _ => i += 1,
        }
    }
    for open in stack {
        problems.push(format!("тег <{open}> не закрыт"));
    }
    if !unsupported.is_empty() {
        problems.insert(
            0,
            format!(
                "Telegram не поддерживает {}: разрешены только b, i, u, s, a, code, pre, blockquote, tg-spoiler, \
                 переносы строк — обычным \\n (без <p>/<br>); если «<» — часть текста (например Vec<T>), пишите &lt;",
                unsupported.join(", ")
            ),
        );
    }
    let mut seen = std::collections::HashSet::new();
    problems.retain(|p| seen.insert(p.clone()));
    problems
}

fn snippet(s: &str) -> String {
    s.chars().take(24).collect()
}

/// Полная проверка поста перед публикацией: разметка + лимит длины.
/// `has_cover` — пост уйдёт фотографией с подписью (лимит 1024 вместо 4096).
pub fn check_post(title: &str, body_html: &str, has_cover: bool) -> Vec<String> {
    let mut problems = validate(body_html);
    let limit = if has_cover { CAPTION_LIMIT } else { MESSAGE_LIMIT };
    // Заголовок публикуется первой строкой жирным + пустая строка.
    let len = title.chars().count() + 2 + visible_len(body_html);
    if len > limit {
        problems.push(format!(
            "пост слишком длинный: {len} видимых символов при лимите Telegram {limit}{} — сократите текст",
            if has_cover { " (с обложкой пост уходит подписью к фото)" } else { "" }
        ));
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_markup() {
        let html = "<b>Rust 1.90</b> вышел!\n<a href=\"https://blog.rust-lang.org\">Блог</a> &amp; <code>cargo</code>\n<blockquote>цитата</blockquote>";
        assert!(validate(html).is_empty(), "{:?}", validate(html));
    }

    #[test]
    fn rejects_unsupported_and_unbalanced() {
        assert!(!validate("<p>text</p>").is_empty());
        assert!(!validate("<b>text").is_empty());
        assert!(!validate("a < b").is_empty());
        assert!(!validate("Tom & Jerry").is_empty());
        assert!(!validate("<b><i>x</b></i>").is_empty());
    }

    #[test]
    fn visible_length_ignores_tags_and_entities() {
        assert_eq!(visible_len("<b>ab</b>&lt;c&gt;"), 5);
        assert_eq!(to_plain_text("<a href=\"x\">link</a> &#33;"), "link !");
    }

    #[test]
    fn checks_length_limit() {
        let long = "x".repeat(1100);
        assert!(check_post("t", &long, false).is_empty());
        assert_eq!(check_post("t", &long, true).len(), 1);
    }
}
