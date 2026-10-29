use askama::Template;
use axum::extract::State;
use axum::response::IntoResponse;

use super::format::{day_time, month_short, relative};
use super::inbox::TopicView;
use super::shell::{error_page, render, Shell};
use crate::state::AppState;

pub struct Kpi {
    pub label: &'static str,
    pub value: String,
    pub hint: String,
}

pub struct WeekBar {
    pub label: String,
    pub value: i64,
    pub pct: i64,
    pub tip: String,
    /// Для составного графика «создано»: доли AI и вручную.
    pub ai_pct: i64,
    pub manual_pct: i64,
    pub created_tip: String,
}

pub struct PromptRow {
    pub key: String,
    pub version: String,
    pub total: i64,
    pub pending: i64,
    pub accepted: i64,
    pub rejected: i64,
    pub published: i64,
    /// Доля принятых среди рассмотренных, %
    pub rate: i64,
    pub rate_label: String,
}

pub struct TagBar {
    pub tag: String,
    pub count: i64,
    pub pct: i64,
}

pub struct HeatCell {
    pub level: u8,
    pub tip: String,
}

pub struct HeatRow {
    pub label: &'static str,
    pub cells: Vec<HeatCell>,
}

pub struct FailureView {
    pub article_id: String,
    pub title: String,
    pub target: String,
    pub error: String,
    pub when: String,
}

pub struct SnapshotView {
    pub summary: String,
    pub when: String,
    pub topics: Vec<TopicView>,
}

#[derive(Template)]
#[template(path = "analytics.html")]
struct AnalyticsTemplate {
    shell: Shell,
    kpis: Vec<Kpi>,
    weeks: Vec<WeekBar>,
    week_max: i64,
    created_max: i64,
    funnel_total: i64,
    funnel: Vec<(&'static str, &'static str, i64, i64)>,
    prompts: Vec<PromptRow>,
    tags: Vec<TagBar>,
    heat: Vec<HeatRow>,
    heat_total: i64,
    failures: Vec<FailureView>,
    snapshots: Vec<SnapshotView>,
}

const WEEKDAYS: [&str; 7] = ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"];

pub async fn analytics_page(State(state): State<AppState>) -> axum::response::Response {
    let tz = state.config.timezone;
    let o = match state.analytics_service.overview(12).await {
        Ok(o) => o,
        Err(e) => return error_page(e),
    };

    let reviewed = o.suggestions_accepted + o.suggestions_rejected;
    let acceptance = if reviewed > 0 { format!("{}%", o.suggestions_accepted * 100 / reviewed) } else { "—".into() };
    let kpis = vec![
        Kpi { label: "Опубликовано за 30 дней", value: o.published_last_30d.to_string(), hint: format!("всего {}", o.published_total) },
        Kpi { label: "В очереди публикации", value: o.scheduled_upcoming.to_string(), hint: "запланировано на будущее".into() },
        Kpi { label: "Ждут решения", value: o.suggestions_pending.to_string(), hint: "предложений от Claude".into() },
        Kpi {
            label: "Принято предложений",
            value: acceptance,
            hint: format!("{} принято · {} отклонено", o.suggestions_accepted, o.suggestions_rejected),
        },
        Kpi {
            label: "Ошибки публикации",
            value: o.failed_tasks.to_string(),
            hint: if o.failed_tasks > 0 { "нужна повторная попытка".into() } else { "всё в порядке".into() },
        },
    ];

    let week_max = o.weeks.iter().map(|w| w.published).max().unwrap_or(0).max(1);
    let created_max = o.weeks.iter().map(|w| w.created_ai + w.created_manual).max().unwrap_or(0).max(1);
    let weeks = o
        .weeks
        .iter()
        .map(|w| {
            let end = w.week_start + chrono::Duration::days(6);
            let range = format!(
                "{} {} – {} {}",
                chrono::Datelike::day(&w.week_start),
                month_short(chrono::Datelike::month(&w.week_start)),
                chrono::Datelike::day(&end),
                month_short(chrono::Datelike::month(&end))
            );
            WeekBar {
                label: format!("{} {}", chrono::Datelike::day(&w.week_start), month_short(chrono::Datelike::month(&w.week_start))),
                value: w.published,
                pct: w.published * 100 / week_max,
                tip: format!("{range}: опубликовано {}, неудачных попыток {}", w.published, w.failed),
                ai_pct: w.created_ai * 100 / created_max,
                manual_pct: w.created_manual * 100 / created_max,
                created_tip: format!("{range}: от Claude {}, вручную {}", w.created_ai, w.created_manual),
            }
        })
        .collect();

    let funnel_total = o.suggestions_accepted + o.suggestions_pending + o.suggestions_rejected;
    let funnel = vec![
        ("Принято", "bg-series-1", o.suggestions_accepted, pct(o.suggestions_accepted, funnel_total)),
        ("Ждут решения", "bg-series-3", o.suggestions_pending, pct(o.suggestions_pending, funnel_total)),
        ("Отклонено", "bg-series-2", o.suggestions_rejected, pct(o.suggestions_rejected, funnel_total)),
    ];

    let prompts = o
        .prompts
        .iter()
        .map(|p| {
            let decided = p.accepted + p.rejected;
            PromptRow {
                key: p.key.clone(),
                version: p.version.map(|v| format!("v{v}")).unwrap_or_else(|| "—".into()),
                total: p.total,
                pending: p.pending,
                accepted: p.accepted,
                rejected: p.rejected,
                published: p.published,
                rate: if decided > 0 { p.accepted * 100 / decided } else { 0 },
                rate_label: if decided > 0 { format!("{}%", p.accepted * 100 / decided) } else { "—".into() },
            }
        })
        .collect();

    let tag_max = o.tags.iter().map(|t| t.count).max().unwrap_or(1).max(1);
    let tags = o.tags.iter().map(|t| TagBar { tag: t.label.clone(), count: t.count, pct: t.count * 100 / tag_max }).collect();

    let heat_max = o.publish_hours.iter().map(|h| h.count).max().unwrap_or(0);
    let heat_total = o.publish_hours.iter().map(|h| h.count).sum();
    let heat = (0..7)
        .map(|wd| HeatRow {
            label: WEEKDAYS[wd as usize],
            cells: (0..24)
                .map(|hour| {
                    let count = o.publish_hours.iter().find(|h| h.weekday == wd && h.hour == hour).map(|h| h.count).unwrap_or(0);
                    let level = if count == 0 || heat_max == 0 { 0 } else { (1 + (count * 3 / heat_max)).min(4) as u8 };
                    HeatCell { level, tip: format!("{}, {:02}:00–{:02}:59 — публикаций: {count}", WEEKDAYS[wd as usize], hour, hour) }
                })
                .collect(),
        })
        .collect();

    let failures = o
        .recent_failures
        .iter()
        .map(|f| FailureView {
            article_id: f.article_id.to_string(),
            title: f.title.clone(),
            target: f.target_name.clone(),
            error: f.error.clone().unwrap_or_default(),
            when: day_time(f.updated_at, tz),
        })
        .collect();

    let snapshots = state
        .interest_service
        .recent(6)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|s| SnapshotView {
            summary: s.summary,
            when: relative(s.created_at, tz),
            topics: s
                .topics
                .into_iter()
                .take(10)
                .map(|t| TopicView { pct: t.weight.clamp(1, 10) * 10, name: t.name, weight: t.weight, note: t.note.unwrap_or_default() })
                .collect(),
        })
        .collect();

    render(&AnalyticsTemplate {
        shell: Shell::load(&state, "analytics").await,
        kpis,
        weeks,
        week_max,
        created_max,
        funnel_total,
        funnel,
        prompts,
        tags,
        heat,
        heat_total,
        failures,
        snapshots,
    })
    .into_response()
}

fn pct(part: i64, total: i64) -> i64 {
    if total == 0 {
        0
    } else {
        part * 100 / total
    }
}
