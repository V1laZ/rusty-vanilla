mod modules;

use modules::database;
use modules::generate_lb::generate_leaderboard;
use modules::osu_api;

use dotenvy::dotenv;
use std::env;
use std::sync::Mutex;

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
        if !msg.content.starts_with("!") {
            return;
        }

        if msg.content.starts_with("!cs") {
            osu_api::handle_legacy_score_only(&msg.content).await;
            let msg_args: Vec<&str> = msg.content.split_whitespace().collect();

            let beatmap_id = match msg_args.last() {
                Some(id) => id,
                None => {
                    if let Err(e) = msg.reply(&ctx.http, "Usage: !cs <beatmap_id>").await {
                        println!("Error sending message: {:?}", e);
                    }
                    return;
                }
            };

            handle_generate_country_lb(&ctx, &msg, beatmap_id).await;
        }

        if msg.content.starts_with("!rsc") {
            osu_api::handle_legacy_score_only(&msg.content).await;
            let msg_args: Vec<&str> = msg.content.split_whitespace().collect();

            let user_arg = match msg_args.last() {
                Some(last) => {
                    let last_index = msg_args.len() - 1;
                    if last_index > 0 && msg_args[last_index - 1] == "-m" {
                        ""
                    } else if last.starts_with('-') || *last == "!rsc" {
                        ""
                    } else {
                        last
                    }
                }
                None => "",
            };

            let user = match user_arg.is_empty() {
                true => match database::get_user_by_id(msg.author.id.get() as i64).await {
                    Ok(u) => u.osu_id.to_string(),
                    Err(e) => {
                        let error_msg = match e {
                            database::UserError::UserNotFound => "Failed to get user. Did you connect your osu! account with `!connect {osu_id}`?",
                            database::UserError::DatabaseError(e) => &e.to_string(),
                        };

                        if let Err(e) = msg.reply(&ctx.http, error_msg).await {
                            println!("Error sending message: {:?}", e);
                        }
                        return;
                    }
                },

                false => user_arg.to_string(),
            };

            let recent = match osu_api::get_user_recent(&user).await {
                Ok(r) => r,
                Err(e) => {
                    let error_msg = match e {
                        osu_api::OsuApiError::NotFound(e) => &e.to_string(),
                        _ => "An unknown error occured. Please try again later.",
                    };
                    if let Err(e) = msg.reply(&ctx.http, error_msg).await {
                        println!("Error sending message: {:?}", e);
                    }
                    return;
                }
            };

            handle_generate_country_lb(&ctx, &msg, &recent.beatmap.id.to_string()).await;
        }

        if msg.content.starts_with("!connect") {
            let msg_args: Vec<&str> = msg.content.split_whitespace().collect();

            let osu_id = match msg_args.last() {
                Some(id) => match id.parse::<i64>() {
                    Ok(parsed_id) => parsed_id,
                    Err(_) => {
                        if let Err(e) = msg
                            .reply(&ctx.http, "Invalid osu! ID. Please provide a valid number.")
                            .await
                        {
                            println!("Error sending message: {:?}", e);
                        }
                        return;
                    }
                },
                None => {
                    if let Err(e) = msg.reply(&ctx.http, "Usage: !connect <osu_id>").await {
                        println!("Error sending message: {:?}", e);
                    }
                    return;
                }
            };

            match database::insert_user(msg.author.id.get() as i64, &msg.author.name, osu_id).await
            {
                Ok(_) => {
                    if let Err(e) = msg
                        .reply(&ctx.http, "Successfully connected your osu! account")
                        .await
                    {
                        println!("Error sending message: {:?}", e);
                    }
                }
                Err(e) => {
                    let error_msg = match e {
                        sqlx::Error::Database(db_error)
                            if db_error.code() == Some("1555".into()) =>
                        {
                            "This Discord account is already connected to an osu! account"
                        }
                        _ => "An unknown error occured. Please try again later.",
                    };

                    if let Err(e) = msg.reply(&ctx.http, error_msg).await {
                        println!("Error sending message: {:?}", e);
                    }
                    return;
                }
            }
        }
    }
}

async fn handle_generate_country_lb(ctx: &Context, msg: &Message, beatmap_id: &str) {
    let mut scores = match osu_api::fetch_country_scores(&beatmap_id).await {
        Ok(s) => s,
        Err(e) => {
            let error_msg = match e {
                osu_api::OsuApiError::RequestFailed(_e) => {
                    "Failed to fetch scores. Check if the beatmap ID is correct."
                }
                osu_api::OsuApiError::NotFound(e) => &e.to_string(),
                _ => "An unknown error occured. Please try again later.",
            };
            if let Err(e) = msg.reply(&ctx.http, error_msg).await {
                println!("Error sending message: {:?}", e);
            }
            return;
        }
    };

    let msg_tokens = msg.content.split(" ").collect::<Vec<&str>>();
    let mut mods = String::new();

    for (i, token) in msg_tokens.iter().enumerate() {
        if *token == "-m" && i + 1 < msg_tokens.len() {
            mods = msg_tokens[i + 1].to_string().to_uppercase();
            break;
        }
    }

    if !mods.is_empty() {
        let valid_mods = vec!["HD", "HR", "DT", "NC", "FL", "EZ", "HT", "SO", "NF"];
        let filter_mods = get_mods_without_cl(&mods);

        let invalid_mods: Vec<&String> = filter_mods
            .iter()
            .filter(|chunk| !valid_mods.contains(&chunk.as_str()))
            .collect();

        if !invalid_mods.is_empty() {
            if let Err(e) = msg
                .reply(&ctx.http, format!("Invalid mods {:?}", invalid_mods))
                .await
            {
                println!("Error sending message: {:?}", e);
            }
            return;
        }

        scores.retain(|score| {
            let score_mods = score
                .mods
                .iter()
                .map(|m| m.acronym.clone())
                .collect::<Vec<String>>()
                .join("");

            let mods_without_cl = get_mods_without_cl(&score_mods);

            mods_without_cl == filter_mods
        });

        if scores.is_empty() {
            if let Err(e) = msg
                .reply(&ctx.http, "No scores found with the specified mods")
                .await
            {
                println!("Error sending message: {:?}", e);
            }
            return;
        }
    }

    scores.truncate(7);

    let beatmap_info = match osu_api::fetch_beatmap_info(&beatmap_id).await {
        Ok(b) => b,
        Err(e) => {
            let error_msg = match e {
                osu_api::OsuApiError::RequestFailed(_e) => {
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

    let avatars = match osu_api::get_avatars_bytes_array(&scores).await {
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
        .reference_message(&*msg);

    if let Err(e) = msg.channel_id.send_message(&ctx.http, msg_builder).await {
        println!("Error sending message: {:?}", e);
    }
}

fn get_mods_without_cl(mods: &str) -> std::collections::HashSet<String> {
    mods.chars()
        .collect::<Vec<char>>()
        .chunks(2)
        .map(|c| c.iter().collect::<String>())
        .filter(|s| s != "CL")
        .collect()
}

#[tokio::main]
async fn main() {
    osu_api::LEGACY_SCORE_ONLY.get_or_init(|| Mutex::new(true));
    dotenv().ok();
    let dc_token = env::var("BOT_TOKEN").expect("Missing Discord bot token");

    match osu_api::set_legacy_score_only(true).await {
        Ok(_) => {}
        Err(e) => {
            println!("{}", e);
            return;
        }
    }

    if let Err(e) = database::initialize_db().await {
        println!("Failed to initialize database: {}", e);
        return;
    }

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&dc_token, intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
