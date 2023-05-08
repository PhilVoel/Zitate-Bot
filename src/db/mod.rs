use crate::get_percentage;
use serde::{Serialize, Deserialize};
use surrealdb::{Surreal, engine::remote::ws::Client as SurrealClient};

use crate::{logging::log, QAType, RankingType};

#[derive(Serialize, Deserialize)]
pub struct DbUser {
    pub id: String,
    pub name: String,
    uids: Vec<u64>,
}

impl DbUser {
    pub fn new(id: u64, name: String) -> Self {
        Self {
            id: format!("user:{id}"),
            name,
            uids: vec![id],
        }
    }
}

#[derive(Deserialize)]
struct RankingResult {
    pub name: String,
    pub count: u16,
}

pub static DB: Surreal<SurrealClient> = Surreal::init();

pub async fn add_qa(r#type: QAType, user: DbUser, id: u64) -> String {
    let user_id = user.id;
    let already_said: Option<bool> = DB
        .query(format!("SELECT * FROM zitat:{id} IN (SELECT ->said.out as res FROM {user_id})"))
        .await
        .unwrap()
        .take(0)
        .unwrap();
    let already_said = already_said.unwrap();
    let already_assisted: Option<bool> = DB
        .query(format!("SELECT * FROM zitat:{id} IN (SELECT ->assisted.out as res FROM {user_id})"))
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
    DB.query(format!("RELATE {user_id}->{table_name}->zitat:{id}"))
        .await
        .unwrap();
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
    let ranking: Vec<RankingResult>  = DB.query("SELECT count(), in.name as name FROM type::table($kategorie) GROUP BY name ORDER BY count DESC").bind(("kategorie", type_db)).await.unwrap().take(0).unwrap();
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

pub async fn get_user_stats(user: DbUser) -> String {
    let user_id = user.id;
    let said: Option<i32> = DB
        .query(format!("SELECT count(->said) FROM {user_id}"))
        .await
        .unwrap()
        .take((0, "count"))
        .unwrap();
    let wrote: Option<i32> = DB
        .query(format!("SELECT count(->wrote) FROM {user_id}"))
        .await
        .unwrap()
        .take((0, "count"))
        .unwrap();
    let assisted: Option<i32> = DB
        .query(format!("SELECT count(->assisted) FROM {user_id}"))
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
        "Stats für {}:\nGesagt: {said} ({}%)\nGeschrieben: {wrote} ({}%)\nAssisted: {assisted} ({}%)",
        user.name,
        get_percentage(&said),
        get_percentage(&wrote),
        get_percentage(&assisted)
    )
}

pub async fn get_user_from_db_by_uid(id: &u64) -> surrealdb::Result<Option<DbUser>> {
    Ok(DB
        .query("SELECT name, uids, type::string(id) as id FROM user WHERE $id IN uids")
        .bind(("id", id))
        .await?
        .take(0)?)
}

pub async fn get_user_from_db_by_name(name: &str) -> surrealdb::Result<Option<DbUser>> {
    Ok(DB
        .query("SELECT name, uids, type::string(id) as id FROM user WHERE name = $name")
        .bind(("name", name))
        .await?
        .take(0)?)
}

pub async fn add_user(id: &u64, name: &str) {
    DB.query("CREATE type::thing('user', $id) SET name=$name, uids=[$id]")
        .bind(("name", name))
        .bind(("id", id))
        .await
        .unwrap();
    log(&format!("Added {name} to DB"), "INFO");
}
