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
    Ok(DB
        .query("SELECT name, uids, type::string(id) as id FROM user WHERE $id IN uids")
        .bind(("id", id))
        .await?
        .take(0)?)
}

async fn get_by_name(name: &str) -> surrealdb::Result<Option<User>> {
    Ok(DB
        .query("SELECT name, uids, type::string(id) as id FROM user WHERE name = $name")
        .bind(("name", name))
        .await?
        .take(0)?)
}

impl<'a> Into<Identifier<'a>> for &'a u64 {
    fn into(self) -> Identifier<'a> {
        Identifier::Id(self)
    }
}
impl<'a> Into<Identifier<'a>> for &'a String {
    fn into(self) -> Identifier<'a> {
        Identifier::Name(self)
    }
}

pub async fn get<'a, T>(user: T) -> surrealdb::Result<Option<User>> 
    where T: Into<Identifier<'a>> {
        match user.into() {
            Identifier::Id(id) => get_by_uid(id).await,
            Identifier::Name(name) => get_by_name(name).await,
        }
}

pub async fn add(id: &u64, name: &str) {
    DB.query("CREATE type::thing('user', $id) SET name=$name, uids=[$id]")
        .bind(("name", name))
        .bind(("id", id))
        .await
        .unwrap();
    log(&format!("Added {name} to DB"), "INFO");
}

pub async fn get_stats(user: User) -> String {
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
