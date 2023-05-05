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
            MessageId,
            GuildId,
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
    env,
    time::{
        SystemTime,
        UNIX_EPOCH
    },
    io::{
        Write,
        self
    },
    fs::{
        OpenOptions,
        self
    }
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

#[derive(Deserialize)]
struct RankingResult {
    pub name: String,
    pub count: u16
}

enum RankingType {
    Said,
    Wrote,
    Assisted
}

static mut START_TIME: u128 = 0;
static mut OVERALL_ZITATE_COUNT: u16 = 0;
static DB: Surreal<SurrealClient> = Surreal::init();

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, _: Ready) {
        log("Logged in", "INFO");
        loop {
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();
            console_input_handler(input, &ctx, &self.config).await;
        }
    }

    async fn message(&self, ctx: Context, msg: Message) {
        let config = &self.config;
        let zitate_channel_id = *config.get_unsigned("channelZitate");
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

    async fn message_delete(&self, ctx: Context, channel_id: ChannelId, msg_id: MessageId, _: Option<GuildId>) {
        let config = &self.config;
        if *channel_id.as_u64() == *config.get_unsigned("channelZitate") {
            remove_zitat(msg_id, channel_id, &ctx).await;
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
        let overall_num_zitate: Option<u16> = DB.query("SELECT count() FROM zitat GROUP BY count").await.unwrap().take((0, "count")).unwrap();
        OVERALL_ZITATE_COUNT = match overall_num_zitate {
            Some(num) => num,
            None => 0
        };
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

async fn console_input_handler(input: String, ctx: &Context, config: &pml::PmlStruct) {
    let input = input.trim();
    log_to_file(format!("[{}] > {}", get_date_string(), input));
    let result: Vec<String> = input.split(" ").map(|s| s.to_string()).collect();
    match result.get(0) {
        Some(s) if s == "zitat" => match result.get(1) {
            Some(s) if s == "add" => register_zitat(fetch_message_from_id(result.get(2).unwrap().parse::<u64>().unwrap(), *config.get_unsigned("channelZitate"), ctx).await.unwrap(), config, ctx).await,
            Some(s) if s == "remove" => remove_zitat(MessageId(result.get(2).unwrap().parse::<u64>().unwrap()), ChannelId(*config.get_unsigned("channelZitate")), ctx).await,
            Some(_) => println!("Unknown subcommand"),
            None => println!("Missing subcommand")
        },
        Some(s) if s == "user" => match result.get(1) {
            Some(s) if s == "add" => {
                add_user(&result.get(3).unwrap().parse::<u64>().unwrap(), result.get(2).unwrap()).await;
            },
            Some(s) if s == "stats" => {
                match get_user_from_db_by_name(result.get(2).unwrap()).await.unwrap() {
                    Some(user) => println!("{}", get_user_stats(user).await),
                    None => println!("User not found")
                }
            },
            Some(s) if s == "ranking" => {
                let r#type = match result.get(2) {
                    Some(s) if s == "said" => RankingType::Said,
                    Some(s) if s == "wrote" => RankingType::Wrote,
                    Some(s) if s == "assisted" => RankingType::Assisted,
                    Some(_) => {
                        println!("Unknown ranking type");
                        return;
                    },
                    None => {
                        println!("Missing ranking type");
                        return;
                    }
                };
                println!("{}", get_ranking(r#type).await);
            },
            Some(_) => println!("Unknown subcommand"),
            None => println!("Missing subcommand")
        },
        Some(s) if s == "exit" => {
            ctx.shard.shutdown_clean();
            if env::args().collect::<Vec<String>>().contains(&"test".to_string()) {
                fs::remove_file(get_log_file_path()).unwrap();
            }
            else {
                log("Exiting...", "INFO");
            }
            std::process::exit(0);
        }
        Some(_) => println!("Unknown command"),
        None => (),
    }
}

fn get_percentage(count: &u16) -> f32 {
    let total;
    unsafe {
        total = OVERALL_ZITATE_COUNT;
    }
    (*count as f32 * 10_000.0 / total as f32).round() / 100.0
}

async fn get_ranking(r#type: RankingType) -> String {
    let (type_de, type_db) = match r#type {
        RankingType::Said => ("gesprochene", "said"),
        RankingType::Wrote => ("geschriebene", "wrote"),
        RankingType::Assisted => ("assistierte", "assisted")
    };
    let ranking: Vec<RankingResult>  = DB.query("SELECT count(), in.name as name FROM type::table($kategorie) GROUP BY name ORDER BY count DESC").bind(("kategorie", type_db)).await.unwrap().take(0).unwrap();
    format!("Ranking {} Zitate:\n{}", type_de, ranking.iter().enumerate().map(|(i, r)| format!("{:02}.: {}: {} ({}%)", i + 1, r.name, r.count, get_percentage(&r.count))).collect::<Vec<String>>().join("\n"))
}

async fn get_user_stats(user: DbUser) -> String {
    let user_id = user.id;
    let said: Option<i32> = DB.query(format!("SELECT count(->said) FROM {}", user_id)).await.unwrap().take((0, "count")).unwrap();
    let wrote: Option<i32> = DB.query(format!("SELECT count(->wrote) FROM {}", user_id)).await.unwrap().take((0, "count")).unwrap();
   let assisted: Option<i32> = DB.query(format!("SELECT count(->assisted) FROM {}", user_id)).await.unwrap().take((0, "count")).unwrap();
   let said: u16 = match said {
        Some(s) => s as u16,
        None => 0
    };
    let wrote: u16 = match wrote {
        Some(s) => s as u16,
        None => 0
    };
    let assisted: u16 = match assisted {
        Some(s) => s as u16,
        None => 0
    };
    format!("Stats für {}:\nGesagt: {} ({}%)\nGeschrieben: {} ({}%)\nAssisted: {} ({}%)",
            user.name, said, get_percentage(&said), wrote, get_percentage(&wrote), assisted, get_percentage(&assisted))
}

async fn fetch_message_from_id(msg_id: u64, channel_id: u64, ctx: &Context) -> Option<Message> {
    if let Some(cache_result) = ctx.cache.message(channel_id, msg_id) {
        Some(cache_result)
    }
    else if let Ok(http_result) = ctx.http.get_message(channel_id, msg_id).await {
        Some(http_result)
    }
    else {
        None
    }
}

async fn remove_zitat(msg_id: MessageId, channel_id: ChannelId, ctx: &Context) {
    log(&format!("Deleting Zitat with ID {}", msg_id.as_u64()), "WARN");
    DB.query(format!("BEGIN TRANSACTION; DELETE zitat:{}; DELETE wrote, said, assisted WHERE out=zitat:{}; COMMIT TRANSACTION", msg_id, msg_id)).await.unwrap();
    if let Some(old_msg) = fetch_message_from_id(*msg_id.as_u64(), *channel_id.as_u64(), ctx).await {
        log(&format!("Content: {}", old_msg.content), "INFO");
        log(&format!("Author:  {}", old_msg.author.name), "INFO");
        log(&format!("Date:    {}", old_msg.timestamp), "INFO");
    }
    else {
        log("Message not found in cache", "WARN");
    }
    log("Deleted from DB", "INFO");
}

fn log(message: &str, r#type: &str) {
    let print_string = format!("[{}] [{}] {}", get_date_string(), r#type, message);
    println!("{}", print_string);
    log_to_file(print_string);
}

fn get_log_file_path() -> String {
    let file_path;
    unsafe {
        file_path = format!("logs/{}.log", START_TIME);
    }
    file_path
}

fn log_to_file(print_string: String) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(get_log_file_path())
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
            add_user(&author_id, &zitat_msg.author.name).await;
            DbUser{
                id: format!("user:{}", author_id),
                name: zitat_msg.author.name.to_string(),
                uids: vec![author_id]
            }
        }
        Err(e) => {
            log(&format!("Error while getting user from db: {}", e), "ERR ");
            add_user(&author_id, &zitat_msg.author.name).await;
            DbUser{
                id: format!("user:{}", author_id),
                name: zitat_msg.author.name.to_string(),
                uids: vec![author_id]
            }
        }
    };
    DB.query(format!("CREATE zitat:{0} SET text=type::string($text); RELATE {1}->wrote->zitat:{0} SET time=type::datetime($time)", msg_id, author.id))
        .bind(("text", &zitat_msg.content))
        .bind(("time", zitat_msg.timestamp))
        .await.unwrap();
    log(&format!("Zitat with ID {} successfully inserted into DB", msg_id), "INFO");
    let channel_id = *config.get_unsigned("channelBot");
    let bot_channel = if let Some(GuildChannel(bot_channel)) = ctx.cache.channel(channel_id) {
        bot_channel
    }
    else if let GuildChannel(bot_channel) = ctx.http.get_channel(channel_id).await.unwrap() {
        bot_channel
    }
    else {
        log("Could not get #zitate-bot", "ERR ");
        return;
    };
    let thread_msg = bot_channel.say(&ctx.http, format!("{}\n{}", zitat_msg.link(), zitat_msg.content)).await.unwrap();
    ChannelId(channel_id).create_public_thread(&ctx.http, thread_msg, |thread| thread.name(msg_id.to_string()).kind(ChannelType::PublicThread)).await.unwrap();
    log("Created thread in #zitate-bot", "INFO");
    unsafe {
        OVERALL_ZITATE_COUNT += 1;
    }
}

async fn add_user(id: &u64, name: &str) {
    DB.query("CREATE type::thing('user', $id) SET name=$name, uids=[$id]")
        .bind(("name", name))
        .bind(("id", id))
        .await.unwrap();
    log(&format!("Added {} to DB", name), "INFO");
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
    if let Some(user) = ctx.cache.user(*id) {
        user.direct_message(&ctx, |m| m.content(&message)).await.unwrap();
    }
    else {
        ctx.http.get_user(*id).await.unwrap().direct_message(&ctx, |m| m.content(&message)).await.unwrap();
    }
}
