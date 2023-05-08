mod create_commands;
use crate::{
    add_qa, delete_qa_thread, dm_handler, get_ranking, get_user_from_db_by_name, get_user_stats,
    logging::log, register_zitat, remove_zitat, QAType, RankingType, DB,
};
use std::{
    env,
    sync::{mpsc, Arc, Mutex},
};

use serenity::{
    async_trait,
    model::{
        application::interaction::Interaction,
        channel::{Channel, Message},
        gateway::Ready,
        id::{ChannelId, GuildId, MessageId},
        prelude::{Activity, MessageType, MessageUpdateEvent},
    },
    prelude::*,
};

pub struct Handler {
    pub config: pml::PmlStruct,
    pub ctx_producer: Arc<Mutex<mpsc::Sender<Context>>>,
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, _: Ready) {
        log("Logged in", "INFO");
        set_status_based_on_start_parameter(&ctx).await;
        GuildId(*self.config.get_unsigned("guildId"))
            .set_application_commands(&ctx.http, |commands| create_commands::create_all(commands))
            .await
            .unwrap();
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
            remove_zitat(msg_id, channel_id, &ctx, config).await;
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
        let zitat_id = event.id.0;
        let old_text: Option<String> = DB
            .query(format!("SELECT text FROM zitat:{zitat_id}"))
            .await
            .unwrap()
            .take((0, "text"))
            .unwrap();
        if old_text == event.content {
            return;
        }
        let new_text = event.content.unwrap();
        log(
            &format!("Changing content of Zitat with ID {zitat_id}:"),
            "INFO",
        );
        log(old_text.as_ref().unwrap(), "INFO");
        log("->", "INFO");
        log(&new_text, "INFO");
        DB.query(format!(
            "UPDATE zitat:{zitat_id} SET text=type::string($text)"
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
                            "SELECT * FROM 0 < (SELECT count(<-said) FROM zitat:{zitat_id}).count"
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

async fn set_status_based_on_start_parameter(ctx: &Context) {
    if env::args()
        .collect::<Vec<String>>()
        .contains(&String::from("quiet"))
    {
        ctx.invisible().await;
    } else {
        ctx.set_activity(Activity::watching("#📃-zitate")).await;
    }
}
