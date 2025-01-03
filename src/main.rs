mod modules;

use modules::database;
use modules::generate_lb::generate_leaderboard;
use modules::osu_api::{
    fetch_beatmap_info, fetch_country_scores, get_avatars_bytes_array, get_user_recent, OsuApiError,
};

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

            handle_generate_country_lb(&ctx, &msg, beatmap_id).await;
        }

        if msg.content.starts_with("!rsc") {
            let msg_args: Vec<&str> = msg.content.split_whitespace().collect();

            let user_arg = match msg_args.get(1) {
                Some(u) => u,
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

            let recent = match get_user_recent(&user).await {
                Ok(r) => r,
                Err(e) => {
                    let error_msg = match e {
                        OsuApiError::NotFound(e) => &e.to_string(),
                        _ => "An unknown error occured. Please try again later.",
                    };
                    if let Err(e) = msg.reply(&ctx.http, error_msg).await {
                        println!("Error sending message: {:?}", e);
                    }
                    return;
                }
            };

            handle_generate_country_lb(&ctx, &msg, &recent.beatmap_id).await;
        }

        if msg.content.starts_with("!connect") {
            let msg_args: Vec<&str> = msg.content.split_whitespace().collect();

            let osu_id = match msg_args.get(1) {
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
    let scores = match fetch_country_scores(&beatmap_id).await {
        Ok(s) => s,
        Err(e) => {
            let error_msg = match e {
                OsuApiError::RequestFailed(_e) => {
                    "Failed to fetch scores. Check if the beatmap ID is correct."
                }
                OsuApiError::NotFound(e) => &e.to_string(),
                _ => "An unknown error occured. Please try again later.",
            };
            if let Err(e) = msg.reply(&ctx.http, error_msg).await {
                println!("Error sending message: {:?}", e);
            }
            return;
        }
    };

    let beatmap_info = match fetch_beatmap_info(&beatmap_id).await {
        Ok(b) => b,
        Err(e) => {
            let error_msg = match e {
                OsuApiError::RequestFailed(_e) => {
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
        .reference_message(&*msg);

    if let Err(e) = msg.channel_id.send_message(&ctx.http, msg_builder).await {
        println!("Error sending message: {:?}", e);
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let dc_token = env::var("BOT_TOKEN").expect("Missing Discord bot token");

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
