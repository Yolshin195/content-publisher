//! Форматирование для шаблонов: русские названия дат, относительное время,
//! статусы постов, сниппеты текста.

use chrono::{DateTime, Datelike, NaiveDate, Utc, Weekday};
use chrono_tz::Tz;
use serde::Serialize;

use crate::domain::{telegram_html, Article, ArticleOrigin, ArticleState};

pub fn weekday_short(w: Weekday) -> &'static str {
    match w {
        Weekday::Mon => "пн",
        Weekday::Tue => "вт",
        Weekday::Wed => "ср",
        Weekday::Thu => "чт",
        Weekday::Fri => "пт",
        Weekday::Sat => "сб",
        Weekday::Sun => "вс",
    }
}

pub fn month_genitive(month: u32) -> &'static str {
    match month {
        1 => "января",
        2 => "февраля",
        3 => "марта",
        4 => "апреля",
        5 => "мая",
        6 => "июня",
        7 => "июля",
        8 => "августа",
        9 => "сентября",
        10 => "октября",
        11 => "ноября",
        12 => "декабря",
        _ => "",
    }
}

pub fn month_short(month: u32) -> &'static str {
    match month {
        1 => "янв",
        2 => "фев",
        3 => "мар",
        4 => "апр",
        5 => "мая",
        6 => "июн",
        7 => "июл",
        8 => "авг",
        9 => "сен",
        10 => "окт",
        11 => "ноя",
        12 => "дек",
        _ => "",
    }
}

pub fn month_nominative(month: u32) -> &'static str {
    match month {
        1 => "Январь",
        2 => "Февраль",
        3 => "Март",
        4 => "Апрель",
        5 => "Май",
        6 => "Июнь",
        7 => "Июль",
        8 => "Август",
        9 => "Сентябрь",
        10 => "Октябрь",
        11 => "Ноябрь",
        12 => "Декабрь",
        _ => "",
    }
}

/// «29 окт, 07:00» (год — только если не текущий).
pub fn day_time(dt: DateTime<Utc>, tz: Tz) -> String {
    let local = dt.with_timezone(&tz);
    let now = Utc::now().with_timezone(&tz);
    if local.year() == now.year() {
        format!("{} {}, {}", local.day(), month_short(local.month()), local.format("%H:%M"))
    } else {
        format!("{} {} {}, {}", local.day(), month_short(local.month()), local.year(), local.format("%H:%M"))
    }
}

/// «29 октября, ср»
pub fn day_caption(date: NaiveDate) -> String {
    format!("{} {}, {}", date.day(), month_genitive(date.month()), weekday_short(date.weekday()))
}

/// «5 мин назад», «вчера», «3 дн назад», иначе дата.
pub fn relative(dt: DateTime<Utc>, tz: Tz) -> String {
    let secs = (Utc::now() - dt).num_seconds();
    match secs {
        s if s < 0 => day_time(dt, tz),
        s if s < 60 => "только что".into(),
        s if s < 3600 => format!("{} мин назад", s / 60),
        s if s < 86_400 => format!("{} ч назад", s / 3600),
        s if s < 2 * 86_400 => "вчера".into(),
        s if s < 7 * 86_400 => format!("{} дн назад", s / 86_400),
        _ => day_time(dt, tz),
    }
}

/// Статус для отображения: учитывает фактический результат публикации.
pub fn display_status(a: &Article) -> (&'static str, &'static str) {
    match a.state {
        ArticleState::Suggested => ("suggested", "Предложение"),
        ArticleState::Draft => ("draft", "Черновик"),
        ArticleState::Rejected => ("rejected", "Отклонён"),
        ArticleState::Archived => ("archived", "В архиве"),
        ArticleState::Scheduled => match a.publication.status() {
            "published" => ("published", "Опубликован"),
            "failed" => ("failed", "Ошибка публикации"),
            "partial" => ("partial", "Опубликован частично"),
            _ => ("scheduled", "Запланирован"),
        },
    }
}

pub fn snippet(html: &str, max_chars: usize) -> String {
    let plain = telegram_html::to_plain_text(html);
    let collapsed: String = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        collapsed
    } else {
        let cut: String = collapsed.chars().take(max_chars).collect();
        format!("{}…", cut.trim_end())
    }
}

/// «1 пост», «3 поста», «11 постов».
pub fn plural_ru(n: i64, one: &str, few: &str, many: &str) -> String {
    let (n10, n100) = (n.abs() % 10, n.abs() % 100);
    let form = if n10 == 1 && n100 != 11 {
        one
    } else if (2..=4).contains(&n10) && !(12..=14).contains(&n100) {
        few
    } else {
        many
    };
    format!("{n} {form}")
}

/// JSON для вставки в `<script type="application/json">` без риска закрыть тег.
pub fn json_for_script<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".into()).replace("</", "<\\/")
}

/// Плоское представление поста для списков, карточек и календаря.
#[derive(Clone)]
pub struct PostView {
    pub id: String,
    pub title: String,
    pub snippet: String,
    pub status: &'static str,
    pub status_label: &'static str,
    pub is_ai: bool,
    pub tags: Vec<String>,
    /// «29 окт, 07:00» или пусто
    pub when: String,
    /// «07:00» в часовом поясе автора или пусто
    pub time: String,
    pub prompt: Option<String>,
    pub prompt_key: String,
    pub rationale: Option<String>,
    pub feedback: Option<String>,
    pub sources: Vec<(String, String)>,
    pub created: String,
    pub updated: String,
    pub permalink: Option<String>,
}

impl PostView {
    pub fn new(a: &Article, tz: Tz, snippet_len: usize) -> Self {
        let (status, status_label) = display_status(a);
        Self {
            id: a.id.to_string(),
            title: a.title.clone(),
            snippet: snippet(&a.content_html, snippet_len),
            status,
            status_label,
            is_ai: a.origin == ArticleOrigin::Ai,
            tags: a.tags.clone(),
            when: a.scheduled_at.map(|d| day_time(d, tz)).unwrap_or_default(),
            time: a.scheduled_at.map(|d| d.with_timezone(&tz).format("%H:%M").to_string()).unwrap_or_default(),
            prompt: a.prompt_key.as_ref().map(|k| match a.prompt_version {
                Some(v) => format!("{k} · v{v}"),
                None => k.clone(),
            }),
            prompt_key: a.prompt_key.clone().unwrap_or_default(),
            rationale: a.rationale.clone(),
            feedback: a.feedback.clone(),
            sources: a
                .sources
                .iter()
                .map(|s| (if s.title.is_empty() { s.url.clone() } else { s.title.clone() }, s.url.clone()))
                .collect(),
            created: relative(a.created_at, tz),
            updated: relative(a.updated_at, tz),
            permalink: a.publication.permalink.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::plural_ru;

    #[test]
    fn russian_plurals() {
        let f = |n| plural_ru(n, "пост", "поста", "постов");
        assert_eq!(f(1), "1 пост");
        assert_eq!(f(3), "3 поста");
        assert_eq!(f(5), "5 постов");
        assert_eq!(f(11), "11 постов");
        assert_eq!(f(21), "21 пост");
        assert_eq!(f(112), "112 постов");
    }
}
