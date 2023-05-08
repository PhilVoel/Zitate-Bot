use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use serenity::{
    async_trait,
    model::{
        application::interaction::Interaction,
        channel::{
            Channel::{self, Guild as GuildChannel},
            Message,
        },
        gateway::Ready,
        id::{ChannelId, GuildId, MessageId, UserId as SerenityUserId},
        prelude::{
            command::CommandOptionType, Activity, ChannelType, GatewayIntents, MessageType,
            MessageUpdateEvent,
        },
    },
    prelude::*,
};
use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    sync::{mpsc, Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use surrealdb::{
    engine::remote::ws::{Client as SurrealClient, Ws},
    opt::auth::Database,
    Surreal,
};

struct Handler {
    pub config: pml::PmlStruct,
    pub ctx_producer: Arc<Mutex<mpsc::Sender<Context>>>,
}

#[derive(Serialize, Deserialize)]
struct DbUser {
    id: String,
    pub name: String,
    uids: Vec<u64>,
}

#[derive(Deserialize)]
struct RankingResult {
    pub name: String,
    pub count: u16,
}

enum RankingType {
    Said,
    Wrote,
    Assisted,
}

#[derive(PartialEq)]
enum QAType {
    Said,
    Assisted,
}

static mut START_TIME: u128 = 0;
static mut OVERALL_ZITATE_COUNT: u16 = 0;
static DB: Surreal<SurrealClient> = Surreal::init();

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, _: Ready) {
        if env::args()
            .collect::<Vec<String>>()
            .contains(&String::from("quiet"))
        {
            ctx.invisible().await;
        } else {
            ctx.set_activity(Activity::watching("#📃-zitate")).await;
        }
        log("Logged in", "INFO");

        GuildId(*self.config.get_unsigned("guildId")).set_application_commands(&ctx.http, |commands| {
            commands
                .create_application_command(|command| command
                                            .name("stats")
                                            .description("Erhalte Statistiken von jemandem")
                                            .create_option(|option| option
                                                           .name("name")
                                                           .description("Der, von dem du die Statistiken willst")
                                                           .kind(CommandOptionType::String)))
                .create_application_command(|command| command
                                            .name("ranking")
                                            .description("Rankt alle Mitglieder nach der Anzahl ihrer gesagten, assistierten oder geschriebenen Zitate")
                                            .create_option(|option| option
                                                           .name("kategorie")
                                                           .description("Die Kategorie, nach der du ranken willst")
                                                           .kind(CommandOptionType::String)
                                                           .required(true)
                                                           .add_string_choice("gesagt", "said")
                                                           .add_string_choice("geschrieben", "wrote")
                                                           .add_string_choice("assistiert", "assisted")))
                .create_application_command(|command| command
                                            .name("gesagt")
                                            .description("Fügt einen Zitierten zum Zitat hinzu")
                                            .create_option(|option| option
                                                           .name("name")
                                                           .description("Der, der das Zitat gesagt hat")
                                                           .kind(CommandOptionType::String)
                                                           .required(true)))
                .create_application_command(|command| command
                                            .name("assistiert")
                                            .description("Fügt einen Assister zum Zitat hinzu")
                                            .create_option(|option| option
                                                           .name("name")
                                                           .description("Der, der einen Assist gemacht hat")
                                                           .kind(CommandOptionType::String)
                                                           .required(true)))
                .create_application_command(|command| command
                                            .name("fertig")
                                            .description("Alle Sager und Assister sind eingetragen; Thread wird gelöscht"))
        }).await.unwrap();

        self.ctx_producer.lock().unwrap().send(ctx).unwrap();
    }

    async fn message(&self, ctx: Context, msg: Message) {
        let config = &self.config;
        let zitate_channel_id = *config.get_unsigned("channelZitate");
        if msg.author.bot || msg.kind != MessageType::Regular {
            return;
        } else if *msg.channel_id.as_u64() == zitate_channel_id {
            register_zitat(msg, config, &ctx).await;
        } else if let Channel::Private(_) = msg.channel(&ctx).await.unwrap() {
            dm_handler(msg, config, &ctx).await;
        }
    }

    async fn message_delete(
        &self,
        ctx: Context,
        channel_id: ChannelId,
        msg_id: MessageId,
        _: Option<GuildId>,
    ) {
        let config = &self.config;
        if *channel_id.as_u64() == *config.get_unsigned("channelZitate") {
            remove_zitat(msg_id, channel_id, &ctx, &self.config).await;
        }
    }

    async fn message_update(
        &self,
        _: Context,
        _: Option<Message>,
        _: Option<Message>,
        event: MessageUpdateEvent,
    ) {
        if *event.channel_id.as_u64() != *self.config.get_unsigned("channelZitate") {
            return;
        }
        let old_text: Option<String> = DB
            .query(format!("SELECT text FROM zitat:{}", event.id.0))
            .await
            .unwrap()
            .take((0, "text"))
            .unwrap();
        if old_text == event.content {
            return;
        }
        let new_text = event.content.unwrap();
        log(
            &format!("Changing content of Zitat with ID {}:", event.id.0),
            "INFO",
        );
        log(old_text.as_ref().unwrap(), "INFO");
        log("->", "INFO");
        log(&new_text, "INFO");
        DB.query(format!(
            "UPDATE zitat:{0} SET text=type::string($text)",
            event.id.0
        ))
        .bind(("text", new_text))
        .await
        .unwrap();
        log("Zitat successfully updated", "INFO");
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let channel_id = *command.channel_id.as_u64();
            let parent_id = match command.channel_id.to_channel(&ctx).await.unwrap() {
                Channel::Guild(channel) => channel.parent_id.unwrap().0,
                _ => return,
            };
            let channel = match command.channel_id.to_channel(&ctx).await.unwrap() {
                Channel::Guild(channel) => channel,
                _ => return,
            };
            let zitat_id = channel.name.parse::<u64>().unwrap();
            let bot_channel_id = *self.config.get_unsigned("channelBot");
            let response_text: String = match command.data.name.as_str() {
                "stats" if channel_id == bot_channel_id => {
                    let user = get_user_from_db_by_name(
                        command
                            .data
                            .options
                            .get(0)
                            .unwrap()
                            .value
                            .as_ref()
                            .unwrap()
                            .as_str()
                            .unwrap(),
                    )
                    .await
                    .unwrap()
                    .unwrap();
                    get_user_stats(user).await
                }
                "ranking" if channel_id == bot_channel_id => {
                    let r#type = match command
                        .data
                        .options
                        .get(0)
                        .unwrap()
                        .value
                        .as_ref()
                        .unwrap()
                        .as_str()
                        .unwrap()
                    {
                        "said" => RankingType::Said,
                        "wrote" => RankingType::Wrote,
                        "assisted" => RankingType::Assisted,
                        _ => return,
                    };
                    get_ranking(r#type).await
                }
                "gesagt" if parent_id == bot_channel_id => {
                    add_qa(
                        QAType::Said,
                        get_user_from_db_by_name(
                            command
                                .data
                                .options
                                .get(0)
                                .unwrap()
                                .value
                                .as_ref()
                                .unwrap()
                                .as_str()
                                .unwrap(),
                        )
                        .await
                        .unwrap()
                        .unwrap(),
                        zitat_id,
                    )
                    .await
                }
                "assistiert" if parent_id == bot_channel_id => {
                    add_qa(
                        QAType::Assisted,
                        get_user_from_db_by_name(
                            command
                                .data
                                .options
                                .get(0)
                                .unwrap()
                                .value
                                .as_ref()
                                .unwrap()
                                .as_str()
                                .unwrap(),
                        )
                        .await
                        .unwrap()
                        .unwrap(),
                        zitat_id,
                    )
                    .await
                }
                "fertig" if parent_id == bot_channel_id => {
                    if DB
                        .query(format!(
                            "SELECT * FROM 0 < (SELECT count(<-said) FROM zitat:{}).count",
                            zitat_id
                        ))
                        .await
                        .unwrap()
                        .take::<Option<bool>>(0)
                        .unwrap()
                        .unwrap()
                    {
                        delete_qa_thread(command.channel_id, &ctx, &self.config).await;
                        return;
                    } else {
                        String::from("Nein, bist du nicht")
                    }
                }
                _ => return,
            };
            command
                .create_interaction_response(ctx.http, |response| {
                    response.interaction_response_data(|message| message.content(response_text))
                })
                .await
                .unwrap();
        }
    }
}

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
    let bot_token = config.get_string("botToken");
    DB.connect::<Ws>(config.get_string("dbUrl")).await.unwrap();
    DB.signin(Database {
        namespace: config.get_string("dbNs"),
        database: config.get_string("dbName"),
        username: config.get_string("dbUser"),
        password: config.get_string("dbPass"),
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
    log_to_file(format!("[{}] > {}", get_date_string(), input));
    let result: Vec<String> = input.split(" ").map(|s| s.to_string()).collect();
    match result.get(0) {
        Some(s) if s == "zitat" => match result.get(1) {
            Some(s) if s == "add" => {
                register_zitat(
                    fetch_message_from_id(
                        result.get(2).unwrap().parse::<u64>().unwrap(),
                        *config.get_unsigned("channelZitate"),
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
                    ChannelId(*config.get_unsigned("channelZitate")),
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

async fn delete_qa_thread(channel: ChannelId, ctx: &Context, config: &pml::PmlStruct) {
    ctx.http.delete_channel(*channel.as_u64()).await.unwrap();
    ctx.http
        .delete_message(*config.get_unsigned("channelBot"), *channel.as_u64())
        .await
        .unwrap();
    log(
        &format!("Deleted Thread for Zitat with ID {}", channel.as_u64()),
        "INFO",
    );
}

async fn add_qa(r#type: QAType, user: DbUser, id: u64) -> String {
    let already_said: Option<bool> = DB
        .query(format!(
            "SELECT * FROM zitat:{} IN (SELECT ->said.out as res FROM {})",
            id, user.id
        ))
        .await
        .unwrap()
        .take(0)
        .unwrap();
    let already_said = already_said.unwrap();
    let already_assisted: Option<bool> = DB
        .query(format!(
            "SELECT * FROM zitat:{} IN (SELECT ->said.out as res FROM {})",
            id, user.id
        ))
        .await
        .unwrap()
        .take(0)
        .unwrap();
    let already_assisted = already_assisted.unwrap();
    if already_said && r#type == QAType::Said || already_assisted && r#type == QAType::Assisted {
        return String::from("Der ist dafür bereits eingetragen.");
    }
    if already_said && r#type == QAType::Assisted {
        return String::from("Der hat schon einen Assist für das Zitat.");
    }
    if already_assisted && r#type == QAType::Said {
        return String::from("Der hat das Zitat schon gesagt.");
    }
    let table_name = match r#type {
        QAType::Said => "said",
        QAType::Assisted => "assisted",
    };
    DB.query(format!("RELATE {}->{}->zitat:{}", user.id, table_name, id))
        .await
        .unwrap();
    log(
        &format!(
            "Added {} to {table_name} of Zitat with ID {id} in DB",
            user.name
        ),
        "INFO",
    );
    format!("{} erfolgreich hinzugefügt.", user.name)
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
        RankingType::Assisted => ("assistierte", "assisted"),
    };
    let ranking: Vec<RankingResult>  = DB.query("SELECT count(), in.name as name FROM type::table($kategorie) GROUP BY name ORDER BY count DESC").bind(("kategorie", type_db)).await.unwrap().take(0).unwrap();
    format!(
        "Ranking {} Zitate:\n{}",
        type_de,
        ranking
            .iter()
            .enumerate()
            .map(|(i, r)| format!(
                "{:02}.: {}: {} ({}%)",
                i + 1,
                r.name,
                r.count,
                get_percentage(&r.count)
            ))
            .collect::<Vec<String>>()
            .join("\n")
    )
}

async fn get_user_stats(user: DbUser) -> String {
    let user_id = user.id;
    let said: Option<i32> = DB
        .query(format!("SELECT count(->said) FROM {}", user_id))
        .await
        .unwrap()
        .take((0, "count"))
        .unwrap();
    let wrote: Option<i32> = DB
        .query(format!("SELECT count(->wrote) FROM {}", user_id))
        .await
        .unwrap()
        .take((0, "count"))
        .unwrap();
    let assisted: Option<i32> = DB
        .query(format!("SELECT count(->assisted) FROM {}", user_id))
        .await
        .unwrap()
        .take((0, "count"))
        .unwrap();
    let said: u16 = match said {
        Some(s) => s as u16,
        None => 0,
    };
    let wrote: u16 = match wrote {
        Some(s) => s as u16,
        None => 0,
    };
    let assisted: u16 = match assisted {
        Some(s) => s as u16,
        None => 0,
    };
    format!(
        "Stats für {}:\nGesagt: {} ({}%)\nGeschrieben: {} ({}%)\nAssisted: {} ({}%)",
        user.name,
        said,
        get_percentage(&said),
        wrote,
        get_percentage(&wrote),
        assisted,
        get_percentage(&assisted)
    )
}

async fn fetch_message_from_id(msg_id: u64, channel_id: u64, ctx: &Context) -> Option<Message> {
    if let Some(cache_result) = ctx.cache.message(channel_id, msg_id) {
        Some(cache_result)
    } else if let Ok(http_result) = ctx.http.get_message(channel_id, msg_id).await {
        Some(http_result)
    } else {
        None
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
        GuildId(*config.get_unsigned("guildId"))
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
            DbUser {
                id: format!("user:{}", author_id),
                name: zitat_msg.author.name.to_string(),
                uids: vec![author_id],
            }
        }
        Err(e) => {
            log(&format!("Error while getting user from db: {}", e), "ERR ");
            add_user(&author_id, &zitat_msg.author.name).await;
            DbUser {
                id: format!("user:{}", author_id),
                name: zitat_msg.author.name.to_string(),
                uids: vec![author_id],
            }
        }
    };
    DB.query(format!("CREATE zitat:{0} SET text=type::string($text); RELATE {1}->wrote->zitat:{0} SET time=type::datetime($time)", msg_id, author.id))
        .bind(("text", &zitat_msg.content))
        .bind(("time", zitat_msg.timestamp))
        .await.unwrap();
    log(
        &format!("Zitat with ID {} successfully inserted into DB", msg_id),
        "INFO",
    );
    let channel_id = *config.get_unsigned("channelBot");
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

async fn add_user(id: &u64, name: &str) {
    DB.query("CREATE type::thing('user', $id) SET name=$name, uids=[$id]")
        .bind(("name", name))
        .bind(("id", id))
        .await
        .unwrap();
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
    send_dm(
        config.get_unsigned("ownerId"),
        format!("DM von {}:\n{}", author, msg.content),
        ctx,
    )
    .await;
}

async fn get_user_from_db_by_uid(id: &u64) -> surrealdb::Result<Option<DbUser>> {
    Ok(DB
        .query("SELECT name, uids, type::string(id) as id FROM user WHERE $id IN uids")
        .bind(("id", id))
        .await?
        .take(0)?)
}

async fn get_user_from_db_by_name(name: &str) -> surrealdb::Result<Option<DbUser>> {
    Ok(DB
        .query("SELECT name, uids, type::string(id) as id FROM user WHERE name = $name")
        .bind(("name", name))
        .await?
        .take(0)?)
}

async fn send_dm(id: &u64, message: String, ctx: &Context) {
    println!("Sending DM to {}: {}", id, message);
    if let Some(user) = ctx.cache.user(*id) {
        user.direct_message(&ctx, |m| m.content(&message))
            .await
            .unwrap();
    } else {
        ctx.http
            .get_user(*id)
            .await
            .unwrap()
            .direct_message(&ctx, |m| m.content(&message))
            .await
            .unwrap();
    }
}
