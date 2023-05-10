use serenity::{
    model::{id::ChannelId, prelude::Message},
    prelude::*,
};
use crate::logging::log;

pub async fn delete_qa_thread(channel: ChannelId, ctx: &Context, config: &pml::PmlStruct) {
    let channel_id = *channel.as_u64();
    ctx.http.delete_channel(channel_id).await.unwrap();
    ctx.http
        .delete_message(*config.get("channelBot"), channel_id)
        .await
        .unwrap();
    log(
        &format!("Deleted Thread for Zitat with ID {channel_id}"),
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

pub async fn send_dm(id: &u64, message: String, ctx: &Context) {
    println!("Sending DM to {id}: {message}");
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
