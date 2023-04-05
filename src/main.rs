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

struct Handler;

static mut CONFIG: pml::PmlStruct = pml::new();

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let zitate_channel_id: u64;
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

    async fn ready(&self, _: Context, _ready: Ready) {
        println!("Logged in");
    }
}

#[tokio::main]
async fn main() {
    let bot_token: &str;
    unsafe {
        CONFIG = pml::parse_file("config");
        bot_token = CONFIG.get_string("botToken");
    }
    let intents =
        GatewayIntents::GUILDS |
        GatewayIntents::GUILD_MESSAGES |
        GatewayIntents::MESSAGE_CONTENT |
        GatewayIntents::DIRECT_MESSAGES;
    let mut client = Client::builder(&bot_token, intents).event_handler(Handler).await.expect("Error creating client");
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}

fn register_zitat(_msg: Message) {
}

fn dm_handler(_msg: Message) {
}
