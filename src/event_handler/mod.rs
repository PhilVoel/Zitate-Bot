mod create_commands;
use crate::{
    db::{add_qa, get_ranking, user, DB},
    discord::{send_dm, delete_qa_thread, set_status_based_on_start_parameter},
    logging::log,
    register_zitat,
    remove_zitat,
    QAType,
    RankingType
};
use std::sync::{mpsc, Arc, Mutex};

use serenity::{
    async_trait,
    model::{
        application::interaction::Interaction,
        //application::interaction::{Interaction, application_command::ApplicationCommandInteraction},
        channel::{Channel, Message},
        gateway::Ready,
        id::{ChannelId, GuildId, MessageId, UserId as SerenityUserId},
        prelude::{MessageType, MessageUpdateEvent},
    },
    prelude::{Context, EventHandler}
};

pub struct Handler {
    pub config: pml::PmlStruct,
    pub ctx_producer: Arc<Mutex<mpsc::Sender<Context>>>,
}

/*trait ApplicationCommandInteractionExt {
    async fn reply(&self, ctx: &Context, content: &str) {
            self
                .create_interaction_response(ctx.http, |response| {
                    response.interaction_response_data(|message| message.content(response_text))
                })
                .await
                .unwrap();
    }
}

impl ApplicationCommandInteractionExt for ApplicationCommandInteraction {
}*/

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, _: Ready) {
        log("Logged in", "INFO");
        set_status_based_on_start_parameter(&ctx).await;
        GuildId(*self.config.get("guildId"))
            .set_application_commands(&ctx.http, |commands| create_commands::create_all(commands))
            .await
            .unwrap();
        self.ctx_producer.lock().unwrap().send(ctx).unwrap();
    }

    async fn message(&self, ctx: Context, msg: Message) {
        let config = &self.config;
        let zitate_channel_id = *config.get::<u64>("channelZitate");
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
        if channel_id.0 == *config.get::<u64>("channelZitate") {
            remove_zitat(msg_id.0, &ctx, config).await;
        }
    }

    async fn message_update(
        &self,
        _: Context,
        _: Option<Message>,
        _: Option<Message>,
        event: MessageUpdateEvent,
    ) {
        if *event.channel_id.as_u64() != *self.config.get::<u64>("channelZitate") {
            return;
        }
        let zitat_id = event.id.0;
        let old_text: Option<String> = DB
            .query(format!("SELECT text FROM zitat:{zitat_id}"))
            .await
            .expect("Seems the DB went down")
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
        .expect("Seems the DB went down");
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
            let bot_channel_id = *self.config.get::<u64>("channelBot");
            let response_text: String = match command.data.name.as_str() {
                "stats" if channel_id == bot_channel_id => {
                    let user = match command.data.options.get(0) {
                        Some(input) => {
                            let input = input.value.as_ref().unwrap().as_str().unwrap();
                            let len = input.len()-1;
                            if input.starts_with("<@") && input.ends_with('>') {
                                let id = input[2..len].parse::<u64>().unwrap();
                                user::get(&id).await
                            } else {
                                user::get(&input.to_string()).await
                            }
                        }
                        None => user::get(&command.user.id.0).await
                    };
                    match user {
                        Ok(option) => match option {
                            Some(user) => user::get_stats(user).await,
                            None => String::from("User not found"),
                        },
                        Err(e) => {
                            log(&format!("Error getting user from DB: {e}"), "ERR ");
                            String::from("Error looking up user in DB")
                        }
                    }
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
                "zitate" if channel_id == bot_channel_id => {
                    let user = match command.data.options.get(0) {
                        Some(input) => {
                            let input = input.value.as_ref().unwrap().as_str().unwrap();
                            let len = input.len()-1;
                            if input.starts_with("<@") && input.ends_with('>') {
                                let id = input[2..len].parse::<u64>().unwrap();
                                user::get(&id).await
                            } else {
                                user::get(&input.to_string()).await
                            }
                        }
                        None => user::get(&command.user.id.0).await
                    };
                    match user {
                        Ok(option) => match option {
                            Some(user) => user::get_zitate(user).await,
                            None => String::from("User not found"),
                        },
                        Err(e) => {
                            log(&format!("Error getting user from DB: {e}"), "ERR ");
                            String::from("Error looking up user in DB")
                        }
                    }
                }
                "gesagt" if parent_id == bot_channel_id => {
                    let zitat_id = channel.name.parse::<u64>().unwrap();
                    let input = command.data.options.get(0).unwrap().value.as_ref().unwrap().as_str().unwrap();
                    let len = input.len()-1;
                    match
                        if input.starts_with("<@") && input.ends_with('>') {
                            let id = input[2..len].parse::<u64>().unwrap();
                            user::get(&id).await
                        } else {
                            user::get(&input.to_string()).await
                        }
                    {
                        Ok(option) => match option {
                            Some(user) => add_qa(QAType::Said, user, zitat_id).await,
                            None => String::from("User not found"),
                        },
                        Err(e) => {
                            log(&format!("Error getting user from DB: {e}"), "ERR ");
                            String::from("Error looking up user in DB")
                        }
                    }
                }
                "assistiert" if parent_id == bot_channel_id => {
                    let zitat_id = channel.name.parse::<u64>().unwrap();
                    let input = command.data.options.get(0).unwrap().value.as_ref().unwrap().as_str().unwrap();
                    let len = input.len()-1;
                    match
                        if input.starts_with("<@") && input.ends_with('>') {
                            let id = input[2..len].parse::<u64>().unwrap();
                            user::get(&id).await
                        } else {
                            user::get(&input.to_string()).await
                        }
                    {
                        Ok(option) => match option {
                            Some(user) => add_qa(QAType::Assisted, user, zitat_id).await,
                            None => String::from("User not found"),
                        },
                        Err(e) => {
                            log(&format!("Error getting user from DB: {e}"), "ERR ");
                            String::from("Error looking up user in DB")
                        }
                    }
                }
                "fertig" if parent_id == bot_channel_id => {
                    let zitat_id = channel.name.parse::<u64>().unwrap();
                    if DB
                        .query(format!(
                            "SELECT * FROM 0 < (SELECT count(<-said) FROM zitat:{zitat_id}).count"
                        ))
                        .await
                        .expect("Seems the DB went down")
                        .take::<Option<bool>>(0)
                        .unwrap()
                        .unwrap()
                    {
                        let thread_name = match command.channel_id.to_channel(&ctx).await.unwrap() {
                            Channel::Guild(channel) => channel.name,
                            _ => return,
                        };
                        delete_qa_thread(thread_name, &ctx, &self.config).await;
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

async fn dm_handler(msg: Message, config: &pml::PmlStruct, ctx: &Context) {
    let SerenityUserId(author_id) = msg.author.id;
    if author_id == *config.get::<u64>("ownerId") {
        return;
    }
    let author = match user::get(&author_id).await {
        Ok(Some(user_data)) => user_data.name.to_string(),
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
