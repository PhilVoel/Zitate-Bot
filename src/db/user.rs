use crate::db::get_percentage;
use serde::{Deserialize, Serialize};

use super::DB;
use crate::logging::log;

#[derive(Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub name: String,
    uids: Vec<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ZitateListItem {
    msg_id: String,
    text: String,
}

impl User {
    pub fn new(id: u64, name: String) -> Self {
        Self {
            id: format!("user:{id}"),
            name,
            uids: vec![id],
        }
    }
}

mod id {
    pub enum Identifier<'a> {
        Id(&'a u64),
        Name(&'a String),
    }
}
use id::Identifier;

async fn get_by_uid(id: &u64) -> surrealdb::Result<Option<User>> {
    DB
        .query("SELECT name, uids, type::string(id) as id FROM user WHERE $id IN uids")
        .bind(("id", id))
        .await?
        .take(0)
}

async fn get_by_name(name: &str) -> surrealdb::Result<Option<User>> {
    DB
        .query("SELECT name, uids, type::string(id) as id FROM user WHERE name = $name")
        .bind(("name", name))
        .await?
        .take(0)
}

impl<'a> From<&'a u64> for Identifier<'a> {
    fn from(val: &'a u64) -> Self {
        Identifier::Id(val)
    }
}
impl<'a> From<&'a String> for Identifier<'a> {
    fn from(val: &'a String) -> Self {
        Identifier::Name(val)
    }
}

pub async fn get<'a, T>(user: T) -> surrealdb::Result<Option<User>> 
    where T: Into<Identifier<'a>> {
        match user.into() {
            Identifier::Id(id) => get_by_uid(id).await,
            Identifier::Name(name) => get_by_name(name).await,
        }
}

pub async fn add(id: u64, name: &str) {
    DB.query("CREATE type::thing('user', $id) SET name=$name, uids=[$id]")
        .bind(("name", name))
        .bind(("id", id))
        .await
        .expect("Seems the DB went down");
    log(&format!("Added {name} to DB"), "INFO");
}

pub async fn get_stats(user: User) -> String {
    let user_id = user.id;
    let said: Option<i32> = DB
        .query(format!("SELECT count(->said) FROM {user_id}"))
        .await
        .expect("Seems the DB went down")
        .take((0, "count"))
        .unwrap();
    let wrote: Option<i32> = DB
        .query(format!("SELECT count(->wrote) FROM {user_id}"))
        .await
        .expect("Seems the DB went down")
        .take((0, "count"))
        .unwrap();
    let assisted: Option<i32> = DB
        .query(format!("SELECT count(->assisted) FROM {user_id}"))
        .await
        .expect("Seems the DB went down")
        .take((0, "count"))
        .unwrap();
    let said: u16 = said.unwrap_or(0) as u16; 
    let wrote: u16 = wrote.unwrap_or(0) as u16;
    let assisted: u16 = assisted.unwrap_or(0) as u16;
    format!(
        "Stats für {}:\nGesagt: {said} ({}%)\nGeschrieben: {wrote} ({}%)\nAssisted: {assisted} ({}%)",
        user.name,
        get_percentage(&said),
        get_percentage(&wrote),
        get_percentage(&assisted)
    )
}

pub async fn get_zitate(user: User) -> String {
    let user_id = user.id;
    let zitate: Vec<ZitateListItem> = DB
        .query(format!("SELECT out.text AS text, string::slice(out, 6) AS msg_id FROM {user_id}->said ORDER BY msg_id"))
        .await
        .expect("Seems the DB went down")
        .take(0)
        .unwrap();
    format!("{}", zitate.iter().map(|e| format!("{}\nhttps://discord.com/channels/422796692899758091/528316171389239296/{}", e.text, e.msg_id)).collect::<Vec<String>>().join("\n------------------\n"))
}
