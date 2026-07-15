//! Реализация публикации через Telegram Bot API поверх библиотеки `teloxide`.
//! Бот должен быть администратором целевого канала/группы (даётся заранее вручную).
//!
//! Допущение: `content.body_html` уже содержит HTML-разметку, совместимую с
//! ограниченным набором тегов, который понимает Telegram (`b`, `i`, `u`, `s`,
//! `a`, `code`, `pre` и несколько других) — см.
//! https://core.telegram.org/bots/api#html-style. Полная санитизация
//! произвольного HTML вынесена за рамки этого сервиса.

use teloxide::prelude::*;
use teloxide::types::{ChatId, InlineKeyboardButton, InlineKeyboardMarkup, InputFile, ParseMode, Recipient};

use shared_contracts::{PublishRequest, PublishResult, PublishStatus, VideoPlatform};

/// Лимиты Telegram: полное сообщение — 4096 символов, подпись к фото — 1024.
const TELEGRAM_MESSAGE_LIMIT: usize = 4096;
const TELEGRAM_CAPTION_LIMIT: usize = 1024;

pub async fn publish(bot: &Bot, request: PublishRequest) -> PublishResult {
    match try_publish(bot, &request).await {
        Ok((message_id, permalink)) => PublishResult {
            task_id: request.task_id,
            status: PublishStatus::Success,
            external_post_id: Some(message_id.to_string()),
            permalink,
            error: None,
        },
        Err(error) => PublishResult {
            task_id: request.task_id,
            status: PublishStatus::Failure,
            external_post_id: None,
            permalink: None,
            error: Some(error),
        },
    }
}

async fn try_publish(bot: &Bot, request: &PublishRequest) -> Result<(i32, Option<String>), String> {
    let recipient = parse_recipient(&request.target.external_id)?;
    let parse_mode = parse_mode_from_config(&request.target.config);
    let text = build_text(request, parse_mode);
    let keyboard = build_keyboard(request);

    let message_id = if let Some(cover_url) = &request.content.cover_image_url {
        send_with_photo(bot, recipient.clone(), cover_url, &text, parse_mode, keyboard).await?
    } else {
        send_text_only(bot, recipient.clone(), &text, parse_mode, keyboard).await?
    };

    let permalink = build_permalink(&request.target.external_id, message_id);
    Ok((message_id, permalink))
}

/// `external_id` — либо публичный юзернейм канала (`@channel`), либо числовой
/// chat_id супергруппы/канала (например, `-1001234567890`).
fn parse_recipient(external_id: &str) -> Result<Recipient, String> {
    if let Some(username) = external_id.strip_prefix('@') {
        return Ok(Recipient::ChannelUsername(format!("@{username}")));
    }
    external_id
        .parse::<i64>()
        .map(|id| Recipient::Id(ChatId(id)))
        .map_err(|_| format!("invalid external_id '{external_id}': expected @username or numeric chat id"))
}

fn parse_mode_from_config(config: &serde_json::Value) -> ParseMode {
    match config.get("parse_mode").and_then(|v| v.as_str()) {
        Some(v) if v.eq_ignore_ascii_case("markdownv2") => ParseMode::MarkdownV2,
        Some(v) if v.eq_ignore_ascii_case("markdown") => ParseMode::Markdown,
        _ => ParseMode::Html,
    }
}

fn build_text(request: &PublishRequest, parse_mode: ParseMode) -> String {
    match parse_mode {
        ParseMode::Html => {
            format!("<b>{}</b>\n\n{}", escape_html(&request.content.title), request.content.body_html)
        }
        _ => format!("{}\n\n{}", request.content.title, request.content.body_html),
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Подпись под каждой видео-ссылкой отражает платформу — это и есть та самая
/// поддержка нескольких зеркал одного ролика (YouTube + альтернативный хостинг).
fn video_label(platform: VideoPlatform) -> &'static str {
    match platform {
        VideoPlatform::Youtube => "▶️ Смотреть на YouTube",
        VideoPlatform::VkVideo => "▶️ Смотреть в VK Video",
        VideoPlatform::Rutube => "▶️ Смотреть на Rutube",
        VideoPlatform::Vimeo => "▶️ Смотреть на Vimeo",
        VideoPlatform::Other => "▶️ Смотреть видео",
    }
}

fn build_keyboard(request: &PublishRequest) -> Option<InlineKeyboardMarkup> {
    if request.content.video_links.is_empty() {
        return None;
    }

    let rows: Vec<Vec<InlineKeyboardButton>> = request
        .content
        .video_links
        .iter()
        .filter_map(|link| {
            url::Url::parse(&link.url).ok().map(|url| vec![InlineKeyboardButton::url(video_label(link.platform), url)])
        })
        .collect();

    if rows.is_empty() {
        None
    } else {
        Some(InlineKeyboardMarkup::new(rows))
    }
}

fn truncate(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(limit.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

async fn send_text_only(
    bot: &Bot,
    recipient: Recipient,
    text: &str,
    parse_mode: ParseMode,
    keyboard: Option<InlineKeyboardMarkup>,
) -> Result<i32, String> {
    let text = truncate(text, TELEGRAM_MESSAGE_LIMIT);
    let mut request = bot.send_message(recipient, text).parse_mode(parse_mode);
    if let Some(keyboard) = keyboard {
        request = request.reply_markup(keyboard);
    }
    let message = request.await.map_err(|e| e.to_string())?;
    Ok(message.id.0)
}

async fn send_with_photo(
    bot: &Bot,
    recipient: Recipient,
    photo_url: &str,
    caption: &str,
    parse_mode: ParseMode,
    keyboard: Option<InlineKeyboardMarkup>,
) -> Result<i32, String> {
    let photo_url = url::Url::parse(photo_url).map_err(|e| format!("invalid cover_image_url: {e}"))?;
    let caption = truncate(caption, TELEGRAM_CAPTION_LIMIT);

    let mut request = bot.send_photo(recipient, InputFile::url(photo_url)).caption(caption).parse_mode(parse_mode);
    if let Some(keyboard) = keyboard {
        request = request.reply_markup(keyboard);
    }
    let message = request.await.map_err(|e| e.to_string())?;
    Ok(message.id.0)
}

/// Лучшая попытка построить ссылку на опубликованный пост. Для публичных каналов
/// (`@username`) даёт рабочую ссылку `t.me/username/message_id`. Для приватных
/// супергрупп/каналов с числовым id формата `-100XXXXXXXXXX` — внутреннюю ссылку
/// `t.me/c/XXXXXXXXXX/message_id` (открывается только у тех, кто уже состоит в канале).
fn build_permalink(external_id: &str, message_id: i32) -> Option<String> {
    if let Some(username) = external_id.strip_prefix('@') {
        return Some(format!("https://t.me/{username}/{message_id}"));
    }
    external_id.strip_prefix("-100").map(|internal_id| format!("https://t.me/c/{internal_id}/{message_id}"))
}
