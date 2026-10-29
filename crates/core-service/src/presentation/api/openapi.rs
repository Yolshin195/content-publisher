use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};

use super::{analytics, articles, calendar, interests, media, prompts, targets, tasks};
use crate::application::ports::{AnalyticsOverview, FailureRow, HourBucket, LabelCount, PromptStat, WeekBucket};
use crate::presentation::dto::*;

struct BearerAuth;

impl Modify for BearerAuth {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer",
                SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
            );
        }
    }
}

/// Единая точка сборки OpenAPI-спецификации Core Service. Отдаётся в рантайме на
/// `/api-docs/openapi.json` (и просматривается через Swagger UI на `/docs`), а также
/// может быть сгенерирована статически бинарником `print_openapi` без поднятия БД.
#[derive(OpenApi)]
#[openapi(
    paths(
        articles::list_articles,
        articles::get_article,
        articles::create_article,
        articles::update_article,
        articles::delete_article,
        articles::add_video_link,
        articles::remove_video_link,
        articles::list_tags,
        articles::create_suggestion,
        articles::accept_article,
        articles::reject_article,
        articles::restore_article,
        articles::archive_article,
        targets::list_targets,
        targets::create_target,
        targets::get_target,
        targets::update_target,
        targets::delete_target,
        tasks::schedule_article,
        tasks::unschedule,
        tasks::publish_now,
        tasks::retry_target,
        tasks::cancel_target,
        tasks::list_tasks,
        tasks::task_logs,
        prompts::tree,
        prompts::get_prompt,
        prompts::get_by_key,
        prompts::save_by_key,
        prompts::create,
        prompts::update,
        prompts::move_node,
        prompts::delete,
        prompts::versions,
        prompts::diff,
        prompts::restore,
        prompts::export_one,
        prompts::export_all,
        prompts::import,
        interests::list,
        interests::create,
        analytics::overview,
        calendar::month_view,
        calendar::week_view,
        calendar::day_view,
        media::create_upload_url,
    ),
    components(schemas(
        CreateArticleRequest,
        UpdateArticleRequest,
        AddVideoLinkRequest,
        ScheduleArticleRequest,
        PublishNowRequest,
        ReviewRequest,
        CreateSuggestionRequest,
        SourceLinkDto,
        CreateTargetRequest,
        UpdateTargetRequest,
        VideoLinkResponse,
        ArticleResponse,
        PromptRefDto,
        PublicationSummaryDto,
        TargetResponse,
        TaskResponse,
        LogResponse,
        DayArticleResponse,
        DaySummaryResponse,
        Meta,
        ListArticlesResponse,
        TagCountResponse,
        CreatePromptRequest,
        UpdatePromptRequest,
        SavePromptByKeyRequest,
        MovePromptRequest,
        ImportPromptsRequest,
        PromptResponse,
        PromptTreeResponse,
        ResolvedPromptResponse,
        PromptVersionResponse,
        DiffLineResponse,
        PromptFileResponse,
        InterestTopicDto,
        CreateInterestSnapshotRequest,
        InterestTopicResponse,
        InterestSnapshotResponse,
        AnalyticsOverview,
        LabelCount,
        WeekBucket,
        PromptStat,
        FailureRow,
        HourBucket,
        ErrorResponse,
        ErrorBody,
        shared_contracts::MediaUploadUrlResponse,
    )),
    modifiers(&BearerAuth),
    security(("bearer" = [])),
    tags(
        (name = "articles", description = "CRUD статей и их видео-ссылок"),
        (name = "suggestions", description = "Предложения постов от Claude: создание, принятие, отклонение"),
        (name = "prompts", description = "Справочник промптов: дерево, ключи, версии, diff, Markdown"),
        (name = "interests", description = "Снимки интересов автора, собранные по чатам"),
        (name = "analytics", description = "Сводная аналитика публикаций и предложений"),
        (name = "targets", description = "Целевые площадки публикации (Telegram, VK, ...)"),
        (name = "publication", description = "Планирование, немедленная публикация, повтор и отмена по цели"),
        (name = "calendar", description = "Календарное представление статей (месяц/неделя/день)"),
        (name = "media", description = "Получение presigned URL для загрузки медиафайлов"),
    ),
    info(
        title = "Content Publisher — Core Service API",
        version = "0.2.0",
        description = "REST API сервиса управления статьями и мультиплатформенной публикацией. \
                        Авторизация: `Authorization: Bearer <API_TOKEN>`. Для Claude также доступен \
                        MCP-эндпоинт `/mcp` с теми же возможностями."
    )
)]
pub struct ApiDoc;
