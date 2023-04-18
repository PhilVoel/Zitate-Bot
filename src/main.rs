use serenity::{
    async_trait,
    model::{
        channel::{
            Message,
            Channel::{
                self,
                Guild as GuildChannel
            }
        },
        gateway::Ready,
        id::{
            UserId as SerenityUserId,
            ChannelId
        },
        prelude::{
            ChannelType,
            GatewayIntents
        }
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
use surrealdb::{
    Surreal,
    engine::remote::ws::{
        Ws,Client as SurrealClient
    },
    opt::auth::Database
};
use serde::{
    Serialize,
    Deserialize
};

struct Handler {
    pub config: pml::PmlStruct,
}

#[derive(Serialize, Deserialize)]
struct DbUser {
    id: String,
    pub name: String,
    uids: Vec<u64>
}
static mut START_TIME: u128 = 0;
static DB: Surreal<SurrealClient> = Surreal::init();

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
            register_zitat(msg, config, &ctx).await;
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
    DB.connect::<Ws>(config.get_string("dbUrl")).await.unwrap();
    DB.signin(Database {
        namespace: config.get_string("dbNs"),
        database: config.get_string("dbName"),
        username: config.get_string("dbUser"),
        password: config.get_string("dbPass")
    }).await.unwrap();

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

async fn register_zitat(zitat_msg: Message, config: &pml::PmlStruct, ctx: &Context) {
    let SerenityUserId(author_id) = zitat_msg.author.id;
    let msg_id = zitat_msg.id.as_u64();
    let author = match get_user_from_db_by_uid(&author_id).await {
        Ok(Some(user_data)) => user_data,
        Ok(None) => {
            log("Author not found in DB", "WARN");
            add_user(&author_id, &zitat_msg.author.name).await
        }
        Err(e) => {
            log(&format!("Error while getting user from db: {}", e), "ERR ");
            add_user(&author_id, &zitat_msg.author.name).await
        }
    };
    DB.query(format!("INSERT INTO zitat:{} SET text=type::string({}); RELATE {}->wrote->zitat:{} SET time=type::datetime({})",
        msg_id,
        zitat_msg.content,
        author.id,
        msg_id,
        zitat_msg.timestamp
    )).await.unwrap();
    log(&format!("Zitat with ID {} successfully inserted into DB", msg_id), "INFO");
    if let GuildChannel(bot_channel) = ctx.http.get_channel(*config.get_unsigned("channelBot")).await.unwrap() {
        let thread_msg = bot_channel.say(&ctx.http, format!("{}\n{}", zitat_msg.link(), zitat_msg.content)).await.unwrap();
        ChannelId(*config.get_unsigned("cannelBot")).create_public_thread(&ctx.http, thread_msg, |thread| thread.name(msg_id.to_string()).kind(ChannelType::PublicThread)).await.unwrap();
        log("Created thread in #zitate-bot", "INFO");
    }
}

async fn add_user(id: &u64, name: &str) -> DbUser {
    let entry: DbUser = DB.create(("user", id.to_string())).content(DbUser{
        id: format!("user:{}", id.to_string()),
        name: name.to_string(),
        uids: vec![*id]
    }).await.unwrap();
    log(&format!("Added {} to DB", name), "INFO");
    entry
}

async fn dm_handler(msg: Message, config: &pml::PmlStruct, ctx: &Context) {
    let SerenityUserId(author_id) = msg.author.id;
    if author_id == *config.get_unsigned("ownerId") {
        return;
    }
    let author = match get_user_from_db_by_uid(&author_id).await {
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

async fn get_user_from_db_by_uid(id: &u64) -> surrealdb::Result<Option<DbUser>> {
    Ok(DB.query("SELECT name, uids, type::string(id) as id FROM user WHERE $id IN uids").bind(("id", id)).await?.take(0)?)
}

async fn get_user_from_db_by_name(name: &str) -> surrealdb::Result<Option<DbUser>> {
    Ok(DB.query("SELECT name, uids, type::string(id) as id FROM user WHERE name = $name").bind(("name", name)).await?.take(0)?)
}

async fn send_dm(id: &u64, message: String, ctx: &Context) {
    println!("Sending DM to {}: {}", id, message);
    ctx.http.get_user(*id).await.unwrap().direct_message(&ctx, |m| m.content(&message)).await.unwrap();
}
