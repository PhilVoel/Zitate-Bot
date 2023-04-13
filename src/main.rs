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

struct Handler;

static mut CONFIG: pml::PmlStruct = pml::new();
static mut START_TIME: u128 = 0;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, _: Ready) {
        log("Logged in", "INFO");
    }

    async fn message(&self, ctx: Context, msg: Message) {
        let zitate_channel_id;
        unsafe {
            zitate_channel_id = *CONFIG.get_int("channelZitate") as u64;
        }
        if msg.author.bot || msg.content == "" {
            return;
        }
        else if *msg.channel_id.as_u64() == zitate_channel_id {
            register_zitat(msg);
        }
        else if let Channel::Private(_) = msg.channel(ctx).await.unwrap() {
            dm_handler(msg);
        }
    }
}

#[tokio::main]
async fn main() {
    let bot_token;
    unsafe {
        CONFIG = pml::parse_file("config");
        bot_token = CONFIG.get_string("botToken");
        START_TIME = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
    }
    let intents =
        GatewayIntents::GUILDS |
        GatewayIntents::GUILD_MESSAGES |
        GatewayIntents::MESSAGE_CONTENT |
        GatewayIntents::DIRECT_MESSAGES;
    let mut client = Client::builder(&bot_token, intents).event_handler(Handler).await.expect("Error creating client");
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
    //format!("{:04}.{:02}.{:02} {:02}:{:02}:{:03}", now.year(), now.month(), now.day(), now.hour(), now.minute(), now.second())
}

fn register_zitat(_msg: Message) {
}

fn dm_handler(_msg: Message) {
}
