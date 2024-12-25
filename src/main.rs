mod modules;

use modules::generate_lb::generate_leaderboard;
use modules::osu_api::{fetch_beatmap_info, fetch_country_scores, get_avatars_bytes_array};

use dotenvy::dotenv;
use std::env;

use serenity::all::{CreateAttachment, CreateMessage, Ready};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("🤖 {} is connected and running!", ready.user.name);
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content.starts_with("!cs") {
            let msg_args: Vec<&str> = msg.content.split_whitespace().collect();

            let beatmap_id = match msg_args.get(1) {
                Some(id) => id,
                None => {
                    if let Err(e) = msg.reply(&ctx.http, "Usage: !cs <beatmap_id>").await {
                        println!("Error sending message: {:?}", e);
                    }
                    return;
                }
            };

            let scores = match fetch_country_scores(&beatmap_id).await {
                Ok(s) => s,
                Err(e) => {
                    let error_msg = match e {
                        modules::osu_api::OsuApiError::RequestFailed(e) => {
                            "Failed to fetch scores. Check if the beatmap ID is correct."
                        }
                        _ => "An unknown error occured. Please try again later.",
                    };
                    if let Err(e) = msg.reply(&ctx.http, error_msg).await {
                        println!("Error sending message: {:?}", e);
                    }
                    return;
                }
            };

            if scores.is_empty() {
                if let Err(e) = msg.reply(&ctx.http, "No scores found").await {
                    println!("Error sending message: {:?}", e);
                }
                return;
            }

            let beatmap_info = match fetch_beatmap_info(&beatmap_id).await {
                Ok(b) => b,
                Err(e) => {
                    let error_msg = match e {
                        modules::osu_api::OsuApiError::RequestFailed(e) => {
                            "Failed to fetch beatmap info. Check if the beatmap ID is correct."
                        }
                        _ => "An unknown error occured. Please try again later.",
                    };
                    if let Err(e) = msg.reply(&ctx.http, error_msg).await {
                        println!("Error sending message: {:?}", e);
                    }
                    return;
                }
            };

            let avatars = match get_avatars_bytes_array(&scores).await {
                Ok(a) => a,
                Err(e) => {
                    let error_msg = match e {
                        _ => "An unknown error occured. Please try again later.",
                    };
                    if let Err(e) = msg.reply(&ctx.http, error_msg).await {
                        println!("Error sending message: {:?}", e);
                    }
                    return;
                }
            };

            let table = generate_leaderboard(scores, avatars, &beatmap_info);

            let msg_builder = CreateMessage::new()
                .content(format!(
                    "[**{} - {} [{}]**](<https://osu.ppy.sh/beatmapsets/{}#osu/{}>)\n",
                    beatmap_info.artist,
                    beatmap_info.title,
                    beatmap_info.version,
                    beatmap_info.beatmapset_id,
                    beatmap_info.beatmap_id
                ))
                .add_file(CreateAttachment::bytes(table, "lb.png"))
                .reference_message(&msg);

            if let Err(e) = msg.channel_id.send_message(&ctx.http, msg_builder).await {
                println!("Error sending message: {:?}", e);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let dc_token = env::var("BOT_TOKEN").expect("Missing Discord bot token");

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&dc_token, intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
