use reqwest::Client;
use serde::Deserialize;
use serenity::{all::json, futures::future::join_all};
use std::env;

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

pub async fn fetch_country_scores(beatmap_id: &str) -> Result<Vec<Score>, reqwest::Error> {
    let osu_session = env::var("OSU_SESSION").unwrap();
    let xsrf_token = env::var("XSRF_TOKEN").unwrap();

    let url =
        format!("https://osu.ppy.sh/beatmaps/{beatmap_id}/scores?mode=osu&type=country&limit=7");

    let client = Client::new();
    let response = client
        .get(&url)
        .header("Cookie", format!("osu_session={osu_session}"))
        .header("CSRF-TOKEN", xsrf_token)
        .send()
        .await?
        .text()
        .await?;

    let scores: ScoreResponse = match json::from_str(&response) {
        Ok(s) => s,
        Err(e) => {
            println!("Error parsing JSON: {:?}", e);
            ScoreResponse { scores: vec![] }
        }
    };

    Ok(scores.scores)
}

pub async fn fetch_beatmap_info(beatmap_id: &str) -> Result<Option<Beatmap>, reqwest::Error> {
    let osu_api_key = env::var("OSU_API_KEY").unwrap();
    let url = format!("https://osu.ppy.sh/api/get_beatmaps?k={osu_api_key}&b={beatmap_id}&m=0");

    let client = Client::new();
    let response = client.get(&url).send().await?.text().await?;

    let mut beatmaps: Vec<Beatmap> = match json::from_str(&response) {
        Ok(b) => b,
        Err(e) => {
            println!("Error parsing JSON: {:?}", e);
            vec![]
        }
    };

    if let Some(beatmap) = beatmaps.first_mut() {
        let cover_url = format!(
            "https://assets.ppy.sh/beatmaps/{}/covers/cover.jpg",
            beatmap.beatmapset_id
        );
        if let Ok(cover_response) = client.get(&cover_url).send().await {
            if let Ok(bytes) = cover_response.bytes().await {
                beatmap.cover = bytes.to_vec();
            }
        }
        Ok(Some(beatmap.clone()))
    } else {
        Ok(None)
    }
}

pub async fn get_avatars_bytes_array(scores: &Vec<Score>) -> Vec<Vec<u8>> {
    let futures: Vec<_> = scores
        .iter()
        .map(|s| async move {
            let response = reqwest::get(&s.user.avatar_url).await.unwrap();
            response.bytes().await.unwrap().to_vec()
        })
        .collect();

    join_all(futures).await
}
