use reqwest::Client;
use serde::Deserialize;
use serenity::{all::json, futures::future::join_all};
use std::env;
use std::error::Error;
use std::fmt;

#[derive(Debug, Deserialize)]
struct ScoreResponse {
    scores: Vec<Score>,
}

#[derive(Debug, Deserialize)]
pub struct Score {
    pub legacy_total_score: i64,
    pub ended_at: String,
    pub rank: String,
    pub statistics: Statistics,
    pub mods: Vec<Mod>,
    pub max_combo: i32,
    pub accuracy: f32,
    pub user: User,
}

#[derive(Debug, Deserialize)]
pub struct Statistics {
    #[serde(default)]
    pub ok: Option<i32>,
    #[serde(default)]
    pub miss: Option<i32>,
    #[serde(default)]
    pub meh: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct Mod {
    pub acronym: String,
}

#[derive(Debug, Deserialize)]
pub struct User {
    pub username: String,
    pub avatar_url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Beatmap {
    pub artist: String,
    pub title: String,
    pub version: String,
    pub beatmapset_id: String,
    pub beatmap_id: String,
    pub max_combo: String,
    #[serde(skip)]
    pub cover: Vec<u8>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RecentScore {
    pub beatmap_id: String,
}

#[derive(Debug)]
pub enum OsuApiError {
    RequestFailed(String),
    ParseError(String),
    NotFound(String),
    MissingEnvVar(String),
    ImageError(String),
}

impl fmt::Display for OsuApiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::RequestFailed(msg) => write!(f, "Request failed: {}", msg),
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Self::NotFound(msg) => write!(f, "Not found: {}", msg),
            Self::MissingEnvVar(msg) => write!(f, "Missing environment variable: {}", msg),
            Self::ImageError(msg) => write!(f, "Image error: {}", msg),
        }
    }
}

impl Error for OsuApiError {}

pub async fn fetch_country_scores(beatmap_id: &str) -> Result<Vec<Score>, OsuApiError> {
    let osu_session = env::var("OSU_SESSION")
        .map_err(|_| OsuApiError::MissingEnvVar("OSU_SESSION".to_string()))?;
    let xsrf_token =
        env::var("XSRF_TOKEN").map_err(|_| OsuApiError::MissingEnvVar("XSRF_TOKEN".to_string()))?;

    let url =
        format!("https://osu.ppy.sh/beatmaps/{beatmap_id}/scores?mode=osu&type=country&limit=7");

    let client = Client::new();
    let response = client
        .get(&url)
        .header("Cookie", format!("osu_session={osu_session}"))
        .header("CSRF-TOKEN", xsrf_token)
        .send()
        .await
        .map_err(|e| OsuApiError::RequestFailed(e.to_string()))?
        .text()
        .await
        .map_err(|e| OsuApiError::RequestFailed(e.to_string()))?;

    let scores = json::from_str::<ScoreResponse>(&response)
        .map_err(|e| OsuApiError::ParseError(e.to_string()))?
        .scores;

    if scores.is_empty() {
        return Err(OsuApiError::NotFound(
            "No country scores found for this beatmap".to_string(),
        ));
    }

    Ok(scores)
}

pub async fn fetch_beatmap_info(beatmap_id: &str) -> Result<Beatmap, OsuApiError> {
    let osu_api_key = env::var("OSU_API_KEY")
        .map_err(|_| OsuApiError::MissingEnvVar("OSU_API_KEY".to_string()))?;

    let url = format!("https://osu.ppy.sh/api/get_beatmaps?k={osu_api_key}&b={beatmap_id}&m=0");

    let client = Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| OsuApiError::RequestFailed(e.to_string()))?
        .text()
        .await
        .map_err(|e| OsuApiError::RequestFailed(e.to_string()))?;

    let mut beatmaps: Vec<Beatmap> =
        json::from_str(&response).map_err(|e| OsuApiError::ParseError(e.to_string()))?;

    let beatmap = beatmaps
        .first_mut()
        .ok_or_else(|| OsuApiError::NotFound(format!("Beatmap {} not found", beatmap_id)))?;

    let cover_url = format!(
        "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
        beatmap.beatmapset_id
    );

    let cover_bytes = client
        .get(&cover_url)
        .send()
        .await
        .map_err(|e| OsuApiError::ImageError(e.to_string()))?
        .bytes()
        .await
        .map_err(|e| OsuApiError::ImageError(e.to_string()))?;

    beatmap.cover = cover_bytes.to_vec();
    Ok(beatmap.clone())
}

pub async fn get_avatars_bytes_array(scores: &Vec<Score>) -> Result<Vec<Vec<u8>>, OsuApiError> {
    let futures: Vec<_> = scores
        .iter()
        .map(|s| async {
            let response = reqwest::get(&s.user.avatar_url)
                .await
                .map_err(|e| OsuApiError::ImageError(e.to_string()))?;

            response
                .bytes()
                .await
                .map_err(|e| OsuApiError::ImageError(e.to_string()))
                .map(|b| b.to_vec())
        })
        .collect();

    join_all(futures)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
}

pub async fn get_user_recent(user: &str) -> Result<RecentScore, OsuApiError> {
    let osu_api_key = env::var("OSU_API_KEY")
        .map_err(|_| OsuApiError::MissingEnvVar("OSU_API_KEY".to_string()))?;

    let url =
        format!("https://osu.ppy.sh/api/get_user_recent?k={osu_api_key}&u={user}&m=0&limit=1");

    let client = Client::new();
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| OsuApiError::RequestFailed(e.to_string()))?
        .text()
        .await
        .map_err(|e| OsuApiError::RequestFailed(e.to_string()))?;

    let scores: Vec<RecentScore> =
        json::from_str(&response).map_err(|e| OsuApiError::ParseError(e.to_string()))?;

    scores
        .first()
        .cloned()
        .ok_or_else(|| OsuApiError::NotFound(format!("No recent scores for user {}", user)))
}
