//! Стартовый набор промптов — загружается при первом запуске в пустой справочник.
//! Формат файлов тот же, что у экспорта/импорта: Markdown с front matter.

pub const PROMPTS: &[&str] = &[
    include_str!("../seed/prompts/author_style.md"),
    include_str!("../seed/prompts/telegram_format.md"),
    include_str!("../seed/prompts/news_pipeline.md"),
    include_str!("../seed/prompts/rust_news_research.md"),
    include_str!("../seed/prompts/rust_news_template.md"),
    include_str!("../seed/prompts/chat_digest.md"),
    include_str!("../seed/prompts/chat_post_template.md"),
];
