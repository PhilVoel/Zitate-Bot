use serenity::{
    model::channel::Message,
    prelude::*,
};
use std::{
    env,
    io,
    sync::{mpsc, Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

mod event_handler;
mod logging;
use logging::{log, log_to_file, START_TIME, get_date_string};
mod db;
use db::*;
mod discord;
use discord::*;

pub enum RankingType {
    Said,
    Wrote,
    Assisted,
}

#[derive(PartialEq)]
pub enum QAType {
    Said,
    Assisted,
}

static mut OVERALL_ZITATE_COUNT: u16 = 0;

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
    db::init(&config).await;
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
    let mut client = discord::init_client(config, ctx_producer).await;
    if let Err(why) = client.start().await {
        log(&format!("Could not start client: {:?}", why), "ERR ");
    }
}

async fn console_input_handler(input: String, ctx: &Context, config: &pml::PmlStruct) {
    let input = input.trim();
    log_to_file(format!("[{}] > {input}", get_date_string()));
    let result: Vec<String> = input.split(" ").map(|s| s.to_string()).collect();
    match result.get(0) {
        Some(s) if s == "zitat" => match result.get(1) {
            Some(s) if s == "add" => {
                register_zitat(
                    fetch_message_from_id(
                        result.get(2).unwrap().parse::<u64>().unwrap(),
                        *config.get("channelZitate"),
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
                    result.get(2).unwrap().parse::<u64>().unwrap(),
                    *config.get("channelZitate"),
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
                user::add(
                    &result.get(3).unwrap().parse::<u64>().unwrap(),
                    result.get(2).unwrap(),
                )
                .await;
            }
            Some(s) if s == "stats" => {
                match user::get(result.get(2).unwrap())
                    .await
                    .unwrap()
                {
                    Some(user) => println!("{}", user::get_stats(user).await),
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
                logging::delete();
            } else {
                log("Exiting...", "INFO");
            }
            std::process::exit(0);
        }
        Some(_) => println!("Unknown command"),
        None => (),
    }
}

async fn remove_zitat(
    msg_id: u64,
    channel_id: u64,
    ctx: &Context,
    config: &pml::PmlStruct,
) {
    log(&format!("Deleting Zitat with ID {msg_id}"), "WARN");
    DB.query(format!("DELETE zitat:{msg_id}")).await.unwrap();
    if let Some(old_msg) = fetch_message_from_id(msg_id, channel_id, ctx).await
    {
        log(&format!("Content: {}", old_msg.content), "INFO");
        log(&format!("Author:  {}", old_msg.author.name), "INFO");
        log(&format!("Date:    {}", old_msg.timestamp), "INFO");
    } else {
        log("Message not found in cache", "WARN");
    }
    log("Deleted from DB", "INFO");
    delete_qa_thread(msg_id.to_string(), ctx, config).await;
    unsafe {
        OVERALL_ZITATE_COUNT -= 1;
    }
}

async fn register_zitat(zitat_msg: Message, config: &pml::PmlStruct, ctx: &Context) {
    unsafe {
        OVERALL_ZITATE_COUNT += 1;
    }
    db::insert_zitat(&zitat_msg).await;
    discord::create_qa_thread(&zitat_msg, config, ctx).await;
}
