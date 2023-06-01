use pml::PmlStruct;
use serenity::{
    model::{id::ChannelId, prelude::{Activity, Message, ChannelType, GuildId}, channel::Channel::Guild as GuildChannel},
    prelude::{Context, Client, GatewayIntents},
};
use std::{env, sync::{mpsc::Sender, Arc, Mutex}};
use crate::{logging::log, event_handler::Handler};

pub async fn delete_qa_thread(msg_id: String, ctx: &Context, config: &pml::PmlStruct) {
    let channel = GuildId(config.get("guildId").expect("guildId value not found in config file"))
            .get_active_threads(&ctx.http)
            .await
            .unwrap()
            .threads
            .iter()
            .find(|thread| thread.name() == msg_id)
            .unwrap()
            .id;
    let channel_id = *channel.as_u64();
    ctx.http.delete_channel(channel_id).await.unwrap();
    ctx.http
        .delete_message(config.get("channelBot").expect("channelBot value not found in config file"), channel_id)
        .await
        .unwrap();
    log(
        &format!("Deleted Thread for Zitat with ID {msg_id}"),
        "INFO",
    );
}

pub async fn fetch_message_from_id(msg_id: u64, channel_id: u64, ctx: &Context) -> Option<Message> {
    if let Some(cache_result) = ctx.cache.message(channel_id, msg_id) {
        Some(cache_result)
    } else if let Ok(http_result) = ctx.http.get_message(channel_id, msg_id).await {
        Some(http_result)
    } else {
        None
    }
}

pub async fn send_dm(id: u64, message: String, ctx: &Context) {
    println!("Sending DM to {id}: {message}");
    if let Some(user) = ctx.cache.user(id) {
        user.direct_message(&ctx, |m| m.content(&message))
            .await
            .unwrap();
    } else {
        ctx.http
            .get_user(id)
            .await
            .unwrap()
            .direct_message(&ctx, |m| m.content(&message))
            .await
            .unwrap();
    }
}

pub async fn init_client(config: PmlStruct, ctx_producer: Arc<Mutex<Sender<Context>>>) -> Client {
    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::DIRECT_MESSAGES;
    let bot_token = config.get::<String>("botToken").expect("botToken value not found in config file");
    Client::builder(bot_token, intents)
        .event_handler(Handler {
            config,
            ctx_producer,
        })
        .await
        .expect("Error creating client")
}

pub async fn create_qa_thread(zitat_msg: &Message, config: &pml::PmlStruct, ctx: &Context) {
    let channel_id = config.get("channelBot").expect("channelBot value not found in config file");
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
                .name(zitat_msg.id.as_u64().to_string())
                .kind(ChannelType::PublicThread)
        })
    .await
        .unwrap();
    log("Created thread in #zitate-bot", "INFO");
}

pub async fn set_status_based_on_start_parameter(ctx: &Context) {
    if env::args()
        .collect::<Vec<String>>()
        .contains(&String::from("quiet"))
    {
        ctx.invisible().await;
    } else {
        ctx.set_activity(Activity::watching("#📃-zitate")).await;
    }
}
