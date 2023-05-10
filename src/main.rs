use serenity::{
    model::{
        channel::{
            Channel::Guild as GuildChannel,
            Message,
        },
        id::{ChannelId, GuildId, MessageId, UserId as SerenityUserId},
        prelude::{
            ChannelType, GatewayIntents,
        },
    },
    prelude::*,
};
use std::{
    env,
    fs,
    io,
    sync::{mpsc, Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use surrealdb::{
    engine::remote::ws::Ws,
    opt::auth::Database
};

mod event_handler;
use event_handler::Handler;
mod logging;
use logging::*;
mod db;
use db::*;
mod discord;
use discord::*;

pub enum RankingType {
    Said,
    Wrote,
    Assisted,
}

#[derive(PartialEq)]
pub enum QAType {
    Said,
    Assisted,
}

static mut OVERALL_ZITATE_COUNT: u16 = 0;

#[tokio::main]
async fn main() {
    let (ctx_producer, ctx_receiver) = mpsc::channel();
    let ctx_producer = Arc::new(Mutex::new(ctx_producer));
    tokio::spawn(async move {
        let config = pml::parse_file("config");
        let ctx = ctx_receiver.recv().unwrap();
        loop {
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            console_input_handler(input, &ctx, &config).await;
        }
    });
    let config = pml::parse_file("config");
    let bot_token = config.get::<String>("botToken");
    DB.connect::<Ws>(config.get::<String>("dbUrl").as_str()).await.unwrap();
    DB.signin(Database {
        namespace: config.get::<String>("dbNs"),
        database: config.get::<String>("dbName"),
        username: config.get::<String>("dbUser"),
        password: config.get::<String>("dbPass"),
    })
    .await
    .unwrap();

    unsafe {
        START_TIME = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let overall_num_zitate: Option<u16> = DB
            .query("SELECT count() FROM zitat GROUP BY count")
            .await
            .unwrap()
            .take((0, "count"))
            .unwrap();
        OVERALL_ZITATE_COUNT = match overall_num_zitate {
            Some(num) => num,
            None => 0,
        };
    }
    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::DIRECT_MESSAGES;
    let mut client = Client::builder(&bot_token, intents)
        .event_handler(Handler {
            config,
            ctx_producer,
        })
        .await
        .expect("Error creating client");
    if let Err(why) = client.start().await {
        log(&format!("Could not start client: {:?}", why), "ERR ");
    }
}

async fn console_input_handler(input: String, ctx: &Context, config: &pml::PmlStruct) {
    let input = input.trim();
    log_to_file(format!("[{}] > {input}", get_date_string()));
    let result: Vec<String> = input.split(" ").map(|s| s.to_string()).collect();
    match result.get(0) {
        Some(s) if s == "zitat" => match result.get(1) {
            Some(s) if s == "add" => {
                register_zitat(
                    fetch_message_from_id(
                        result.get(2).unwrap().parse::<u64>().unwrap(),
                        *config.get("channelZitate"),
                        ctx,
                    )
                    .await
                    .unwrap(),
                    config,
                    ctx,
                )
                .await
            }
            Some(s) if s == "remove" => {
                remove_zitat(
                    MessageId(result.get(2).unwrap().parse::<u64>().unwrap()),
                    ChannelId(*config.get("channelZitate")),
                    ctx,
                    config,
                )
                .await
            }
            Some(_) => println!("Unknown subcommand"),
            None => println!("Missing subcommand"),
        },
        Some(s) if s == "user" => match result.get(1) {
            Some(s) if s == "add" => {
                add_user(
                    &result.get(3).unwrap().parse::<u64>().unwrap(),
                    result.get(2).unwrap(),
                )
                .await;
            }
            Some(s) if s == "stats" => {
                match get_user_from_db_by_name(result.get(2).unwrap())
                    .await
                    .unwrap()
                {
                    Some(user) => println!("{}", get_user_stats(user).await),
                    None => println!("User not found"),
                }
            }
            Some(s) if s == "ranking" => {
                let r#type = match result.get(2) {
                    Some(s) if s == "said" => RankingType::Said,
                    Some(s) if s == "wrote" => RankingType::Wrote,
                    Some(s) if s == "assisted" => RankingType::Assisted,
                    Some(_) => {
                        println!("Unknown ranking type");
                        return;
                    }
                    None => {
                        println!("Missing ranking type");
                        return;
                    }
                };
                println!("{}", get_ranking(r#type).await);
            }
            Some(_) => println!("Unknown subcommand"),
            None => println!("Missing subcommand"),
        },
        Some(s) if s == "exit" => {
            ctx.shard.shutdown_clean();
            if env::args()
                .collect::<Vec<String>>()
                .contains(&String::from("test"))
            {
                fs::remove_file(get_log_file_path()).unwrap();
            } else {
                log("Exiting...", "INFO");
            }
            std::process::exit(0);
        }
        Some(_) => println!("Unknown command"),
        None => (),
    }
}

async fn remove_zitat(
    msg_id: MessageId,
    channel_id: ChannelId,
    ctx: &Context,
    config: &pml::PmlStruct,
) {
    log(
        &format!("Deleting Zitat with ID {}", msg_id.as_u64()),
        "WARN",
    );
    DB.query(format!("BEGIN TRANSACTION; DELETE zitat:{}; DELETE wrote, said, assisted WHERE out=zitat:{}; COMMIT TRANSACTION", msg_id, msg_id)).await.unwrap();
    if let Some(old_msg) = fetch_message_from_id(*msg_id.as_u64(), *channel_id.as_u64(), ctx).await
    {
        log(&format!("Content: {}", old_msg.content), "INFO");
        log(&format!("Author:  {}", old_msg.author.name), "INFO");
        log(&format!("Date:    {}", old_msg.timestamp), "INFO");
    } else {
        log("Message not found in cache", "WARN");
    }
    log("Deleted from DB", "INFO");
    delete_qa_thread(
        GuildId(*config.get("guildId"))
            .get_active_threads(&ctx.http)
            .await
            .unwrap()
            .threads
            .iter()
            .find(|thread| thread.name() == msg_id.as_u64().to_string())
            .unwrap()
            .id,
        ctx,
        config,
    )
    .await;
    unsafe {
        OVERALL_ZITATE_COUNT -= 1;
    }
}

async fn register_zitat(zitat_msg: Message, config: &pml::PmlStruct, ctx: &Context) {
    let SerenityUserId(author_id) = zitat_msg.author.id;
    let msg_id = zitat_msg.id.as_u64();
    let author = match get_user_from_db_by_uid(&author_id).await {
        Ok(Some(user_data)) => user_data,
        Ok(None) => {
            log("Author not found in DB", "WARN");
            add_user(&author_id, &zitat_msg.author.name).await;
            db::User::new(author_id, zitat_msg.author.name.clone()) 
        }
        Err(e) => {
            log(&format!("Error while getting user from db: {e}"), "ERR ");
            add_user(&author_id, &zitat_msg.author.name).await;
            db::User::new(author_id, zitat_msg.author.name.clone()) 
        }
    };
    DB.query(format!("CREATE zitat:{msg_id} SET text=type::string($text); RELATE {}->wrote->zitat:{msg_id} SET time=type::datetime($time)", author.id))
        .bind(("text", &zitat_msg.content))
        .bind(("time", zitat_msg.timestamp))
        .await.unwrap();
    log(&format!("Zitat with ID {msg_id} successfully inserted into DB"), "INFO");
    let channel_id = *config.get("channelBot");
    let bot_channel = if let Some(GuildChannel(bot_channel)) = ctx.cache.channel(channel_id) {
        bot_channel
    } else if let GuildChannel(bot_channel) = ctx.http.get_channel(channel_id).await.unwrap() {
        bot_channel
    } else {
        log("Could not get #zitate-bot", "ERR ");
        return;
    };
    let thread_msg = bot_channel
        .say(
            &ctx.http,
            format!("{}\n{}", zitat_msg.link(), zitat_msg.content),
        )
        .await
        .unwrap();
    ChannelId(channel_id)
        .create_public_thread(&ctx.http, thread_msg, |thread| {
            thread
                .name(msg_id.to_string())
                .kind(ChannelType::PublicThread)
        })
        .await
        .unwrap();
    log("Created thread in #zitate-bot", "INFO");
    unsafe {
        OVERALL_ZITATE_COUNT += 1;
    }
}

async fn dm_handler(msg: Message, config: &pml::PmlStruct, ctx: &Context) {
    let SerenityUserId(author_id) = msg.author.id;
    if author_id == *config.get::<u64>("ownerId") {
        return;
    }
    let author = match get_user_from_db_by_uid(&author_id).await {
        Ok(Some(user_data)) => format!("{}", user_data.name),
        Ok(None) => format!("{} (ID: {author_id})", msg.author.tag()),
        Err(e) => {
            log(&format!("Error while getting user from db: {e}"), "ERR ");
            format!("{} (ID: {author_id})", msg.author.tag())
        }
    };
    log(&format!("Received DM from {author}"), "INFO");
    send_dm(
        config.get("ownerId"),
        format!("DM von {author}:\n{}", msg.content),
        ctx,
    )
    .await;
}
