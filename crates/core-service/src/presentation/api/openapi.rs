use utoipa::OpenApi;

use super::{articles, calendar, media, targets, tasks};
use crate::presentation::dto::*;

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
        targets::list_targets,
        targets::create_target,
        targets::get_target,
        targets::update_target,
        targets::delete_target,
        tasks::schedule_article,
        tasks::publish_now,
        tasks::retry_target,
        tasks::cancel_target,
        tasks::list_tasks,
        tasks::task_logs,
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
        CreateTargetRequest,
        UpdateTargetRequest,
        VideoLinkResponse,
        ArticleResponse,
        TargetResponse,
        TaskResponse,
        LogResponse,
        DayArticleResponse,
        DaySummaryResponse,
        Meta,
        ListArticlesResponse,
        ErrorResponse,
        ErrorBody,
        shared_contracts::MediaUploadUrlResponse,
    )),
    tags(
        (name = "articles", description = "CRUD статей и их видео-ссылок"),
        (name = "targets", description = "Целевые площадки публикации (Telegram, VK, ...)"),
        (name = "publication", description = "Планирование, немедленная публикация, повтор и отмена по цели"),
        (name = "calendar", description = "Календарное представление статей (месяц/неделя/день)"),
        (name = "media", description = "Получение presigned URL для загрузки медиафайлов"),
    ),
    info(
        title = "Content Publisher — Core Service API",
        version = "0.1.0",
        description = "REST API сервиса управления статьями и мультиплатформенной публикацией. \
                        Публикация в конкретные соцсети выполняется отдельными Publisher-микросервисами \
                        по общему контракту, описанному в README."
    )
)]
pub struct ApiDoc;
