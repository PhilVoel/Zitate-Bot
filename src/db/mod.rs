use pml::PmlStruct;
use serde::Deserialize;
use serenity::model::{prelude::Message, id::UserId as SerenityUserId};
use surrealdb::{Surreal, engine::remote::ws::{Client as SurrealClient, Ws}, opt::auth::Database};
use once_cell::sync::Lazy;

use crate::{logging::log, QAType, RankingType, OVERALL_ZITATE_COUNT};

pub mod user;
pub use user::User;

#[derive(Deserialize)]
struct RankingResult {
    pub name: String,
    pub count: u16,
}

#[derive(Deserialize)]
struct ZitatDeleteInfo {
    pub content: String,
    pub author_name: String,
    pub timestamp: String
}

pub static DB: Lazy<Surreal<SurrealClient>> = Lazy::new(Surreal::init);

pub async fn add_qa(r#type: QAType, user: User, id: u64) -> String {
    let user_id = user.id;
    let already_said: Option<bool> = DB
        .query(format!("SELECT * FROM zitat:{id} IN (SELECT ->said.out as res FROM {user_id}).res"))
        .await
        .expect("Seems the DB went down")
        .take(0)
        .unwrap();
    let already_said = already_said.unwrap();
    let already_assisted: Option<bool> = DB
        .query(format!("SELECT * FROM zitat:{id} IN (SELECT ->assisted.out as res FROM {user_id}).res"))
        .await
        .expect("Seems the DB went down")
        .take(0)
        .unwrap();
    let already_assisted = already_assisted.unwrap();
    if already_said && r#type == QAType::Said || already_assisted && r#type == QAType::Assisted {
        return String::from("Der ist dafür bereits eingetragen.");
    }
    if already_said && r#type == QAType::Assisted {
        return String::from("Der hat das Zitat schon gesagt.");
    }
    if already_assisted && r#type == QAType::Said {
        return String::from("Der hat schon einen Assist für das Zitat.");
    }
    let table_name = match r#type {
        QAType::Said => "said",
        QAType::Assisted => "assisted",
    };
    DB.query(format!("RELATE {user_id}->{table_name}->zitat:{id}"))
        .await
        .expect("Seems the DB went down");
    let user_name = user.name;
    log(&format!("Added {user_name} to {table_name} of Zitat with ID {id} in DB"), "INFO");
    format!("{user_name} erfolgreich hinzugefügt.")
}

pub async fn get_ranking(r#type: RankingType) -> String {
    let (type_de, type_db) = match r#type {
        RankingType::Said => ("gesprochene", "said"),
        RankingType::Wrote => ("geschriebene", "wrote"),
        RankingType::Assisted => ("assistierte", "assisted"),
    };
    let ranking: Vec<RankingResult>  = DB.query("SELECT count(), in.name as name FROM type::table($kategorie) GROUP BY name ORDER BY count DESC").bind(("kategorie", type_db)).await.expect("Seems the DB went down").take(0).unwrap();
    format!(
        "Ranking {type_de} Zitate:\n{}",
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

pub async fn init(config: &PmlStruct) {
    DB.connect::<Ws>(config.get::<String>("dbUrl").expect("dbUrl value not found in config file").as_str()).await.expect("Error connecting to DB");
    DB.signin(Database {
        namespace: config.get::<&String>("dbNs").expect("dbNs value not found in config file"),
        database: config.get::<&String>("dbName").expect("dbName value not found in config file"),
        username: config.get::<&String>("dbUser").expect("dbUser value not found in config file"),
        password: config.get::<&String>("dbPass").expect("dbPass value not found in config file"),
    })
    .await
    .expect("Error logging into DB");
    log("Connected to database", "INFO");
}


fn get_percentage(count: &u16) -> f32 {
    let total;
    unsafe {
        total = OVERALL_ZITATE_COUNT;
    }
    (*count as f32 * 10_000.0 / total as f32).round() / 100.0
}

pub async fn insert_zitat(zitat_msg: &Message) {
    let SerenityUserId(author_id) = zitat_msg.author.id;
    let msg_id = zitat_msg.id.as_u64();
    let author = match user::get(&author_id).await {
        Ok(Some(user_data)) => user_data,
        Ok(None) => {
            log("Author not found in DB", "WARN");
            user::add(author_id, &zitat_msg.author.name).await;
            User::new(author_id, zitat_msg.author.name.clone()) 
        }
        Err(e) => {
            log(&format!("Error while getting user from db: {e}"), "ERR ");
            user::add(author_id, &zitat_msg.author.name).await;
            User::new(author_id, zitat_msg.author.name.clone()) 
        }
    };
    DB.query(format!("CREATE zitat:{msg_id} SET text=type::string($text); RELATE {}->wrote->zitat:{msg_id} SET time=type::datetime($time)", author.id))
        .bind(("text", &zitat_msg.content.trim()))
        .bind(("time", zitat_msg.timestamp))
        .await.expect("Seems the DB went down");
    log(&format!("Zitat with ID {msg_id} successfully inserted into DB"), "INFO");
}

pub async fn delete_zitat(id: u64) {
    let old_msg: Option<ZitatDeleteInfo> = DB.query(format!("SELECT text as content, time as timestamp, <-said.in.name as author_name FROM zitat:{id}")).await.unwrap().take(0).unwrap();
    let old_msg = old_msg.unwrap();
    log(&format!("Content: {}", old_msg.content), "INFO");
    log(&format!("Author:  {}", old_msg.author_name), "INFO");
    log(&format!("Date:    {}", old_msg.timestamp), "INFO");
    DB.query(format!("DELETE zitat:{id}")).await.expect("Seems the DB went down");
    log("Deleted from DB", "INFO");
}
