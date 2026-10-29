use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Тема, которой автор интересуется в рассматриваемый период.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterestTopic {
    pub name: String,
    /// Вес 1–10: насколько тема заметна в разговорах за период.
    pub weight: i32,
    #[serde(default)]
    pub note: Option<String>,
}

/// Снимок интересов, который Claude формирует по чатам и диалогам автора.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterestSnapshot {
    pub id: Uuid,
    pub source: String,
    pub summary: String,
    pub topics: Vec<InterestTopic>,
    pub period_from: Option<DateTime<Utc>>,
    pub period_to: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewInterestSnapshot {
    pub source: String,
    pub summary: String,
    pub topics: Vec<InterestTopic>,
    pub period_from: Option<DateTime<Utc>>,
    pub period_to: Option<DateTime<Utc>>,
}

/// Тема в динамике по нескольким последним снимкам.
#[derive(Debug, Clone, Serialize)]
pub struct InterestTrend {
    pub name: String,
    pub total_weight: i32,
    pub mentions: usize,
    pub last_weight: i32,
    pub last_seen: DateTime<Utc>,
}

/// Сводит темы нескольких снимков (от новых к старым) в тренд: суммарный вес
/// и последнее появление. Названия сравниваются без учёта регистра.
pub fn build_trends(snapshots: &[InterestSnapshot]) -> Vec<InterestTrend> {
    let mut trends: Vec<InterestTrend> = Vec::new();
    for snap in snapshots {
        for topic in &snap.topics {
            let key = topic.name.trim().to_lowercase();
            if key.is_empty() {
                continue;
            }
            match trends.iter_mut().find(|t| t.name.to_lowercase() == key) {
                Some(t) => {
                    t.total_weight += topic.weight;
                    t.mentions += 1;
                    if snap.created_at > t.last_seen {
                        t.last_seen = snap.created_at;
                        t.last_weight = topic.weight;
                    }
                }
                None => trends.push(InterestTrend {
                    name: topic.name.trim().to_string(),
                    total_weight: topic.weight,
                    mentions: 1,
                    last_weight: topic.weight,
                    last_seen: snap.created_at,
                }),
            }
        }
    }
    trends.sort_by(|a, b| b.total_weight.cmp(&a.total_weight).then(b.last_seen.cmp(&a.last_seen)));
    trends
}
