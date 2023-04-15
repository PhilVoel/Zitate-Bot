use serenity::{
    async_trait,
    model::{
        channel::{
            Message,
            Channel
        },
        gateway::Ready
    },
    prelude::*
};
use std::{
    time::{
        SystemTime,
        UNIX_EPOCH
    },
    io::Write,
    fs::OpenOptions
};
use chrono::prelude::*;

struct Handler {
    pub config: pml::PmlStruct,
}

struct DbUser {
    id: String,
    pub name: String,
    uids: Vec<u64>
}
static mut START_TIME: u128 = 0;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, _: Ready) {
        log("Logged in", "INFO");
    }

    async fn message(&self, ctx: Context, msg: Message) {
        let config = &self.config;
        let zitate_channel_id = *config.get_int("channelZitate") as u64;
        if msg.author.bot || msg.content == "" {
            return;
        }
        else if *msg.channel_id.as_u64() == zitate_channel_id {
            register_zitat(msg);
        }
        else if let Channel::Private(_) = msg.channel(&ctx).await.unwrap() {
            dm_handler(msg, config, &ctx).await;
        }
    }
}

#[tokio::main]
async fn main() {
    let config = pml::parse_file("config");
    let bot_token = config.get_string("botToken");
    unsafe {
        START_TIME = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    }
    let intents =
        GatewayIntents::GUILDS |
        GatewayIntents::GUILD_MESSAGES |
        GatewayIntents::MESSAGE_CONTENT |
        GatewayIntents::DIRECT_MESSAGES;
    let mut client = Client::builder(&bot_token, intents).event_handler(Handler{config}).await.expect("Error creating client");
    if let Err(why) = client.start().await {
        log(&format!("Could not start client: {:?}", why), "ERR ");
    }
}

fn log(message: &str, r#type: &str) {
    let print_string = format!("[{}] [{}] {}", get_date_string(), r#type, message);
    println!("{}", print_string);
    log_to_file(print_string);
}

fn log_to_file(print_string: String) {
    let file_path;
    unsafe {
        file_path = format!("logs/{}.log", START_TIME);
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)
        .unwrap();
    file.write_all(print_string.as_bytes()).unwrap();
}

fn get_date_string() -> String {
    let now = Local::now();
    now.format("%d.%m.%Y %H:%M:%S").to_string()
}

fn register_zitat(_msg: Message) {
}

async fn dm_handler(msg: Message, config: &pml::PmlStruct, ctx: &Context) {
    let serenity::model::id::UserId(author_id) = msg.author.id;
    if author_id == *config.get_unsigned("ownerId") {
        return;
    }
    let author = match get_user_from_db_by_id(&author_id) {
        Ok(Some(user_data)) => format!("{}", user_data.name),
        Ok(None) => format!("{} (ID: {})", msg.author.tag(), author_id),
        Err(e) => {
            log(&format!("Error while getting user from db: {}", e), "ERR ");
            format!("{} (ID: {})", msg.author.tag(), author_id)
        }
    };
    log(&format!("Received DM from {}", author), "INFO");
    send_dm(config.get_unsigned("ownerId"), format!("DM von {}:\n{}", author, msg.content), ctx).await;
}

fn get_user_from_db_by_id(_id: &u64) -> surrealdb::Result<Option<DbUser>> { 
    Ok(None)
}

async fn send_dm(id: &u64, message: String, ctx: &Context) {
    println!("Sending DM to {}: {}", id, message);
    ctx.http.get_user(*id).await.unwrap().direct_message(&ctx, |m| m.content(&message)).await.unwrap();
}
