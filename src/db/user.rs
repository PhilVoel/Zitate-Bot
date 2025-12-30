use crate::{db::{get_percentage, new_connection}, logging::log};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct User {
    pub id: u64,
    pub name: String,
}

impl User {
    pub fn new(id: u64, name: String) -> Self {
        Self {
            id,
            name,
        }
    }
}

pub enum Identifier<'a> {
    Id(&'a u64),
    Name(&'a String),
}

fn get_by_uid(id: &u64) -> Option<User> {
    let connection = new_connection();
    let mut statement = connection.prepare("SELECT u.id AS main_id, u.name AS name
        FROM users as u
        LEFT JOIN other_ids AS o ON u.id = o.main_id
        WHERE u.id = :id OR o.secondary_id = :id").unwrap();
    statement.bind((":id", *id as i64)).unwrap();
    if let sqlite::State::Row = statement.next().unwrap() {
        let id = statement.read::<i64, _>("main_id").unwrap() as u64;
        let name = statement.read::<String, _>("name").unwrap();
        Some(User::new(id, name))
    } else {
        None
    }
}

fn get_by_name(name: &str) -> Option<User> {
    let connection = new_connection();
    let mut statement = connection.prepare("SELECT * from users WHERE name = :name").unwrap();
    let _ = statement.bind((":name", name));
    if let sqlite::State::Row = statement.next().unwrap() {
        let id = statement.read::<i64, _>("id").unwrap() as u64;
        let name = statement.read::<String, _>("name").unwrap();
        Some(User::new(id, name))
    } else {
        None
    }
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

pub fn get<'a, T>(user: T) -> Option<User> 
    where T: Into<Identifier<'a>> {
        match user.into() {
            Identifier::Id(id) => get_by_uid(id),
            Identifier::Name(name) => get_by_name(name),
        }
}

pub fn get_id<'a, T>(user: T) -> Option<u64>
    where T: Into<Identifier<'a>> {
        match user.into() {
            Identifier::Id(id) => Some(*id),
            Identifier::Name(name) => match get_by_name(name) {
                Some(user) => Some(user.id),
                None => None,
            }
        }
}

pub fn add(id: u64, name: &str) {
    let connection = new_connection();
    let mut statement = connection.prepare("INSERT INTO users(id, name) VALUES(:id, :name)").unwrap();
    let _ = statement.bind((":id", id as i64));
    let _ = statement.bind((":name", name));
    let _ = statement.next();
    log(&format!("Added {name} to DB"), "INFO");
}

pub fn get_stats(user: User) -> String {
    let connection = new_connection();

    let mut statement = connection.prepare("SELECT count(user) AS count FROM said WHERE user = :id").unwrap();
    let _ = statement.bind((":id", user.id as i64));
    let _ = statement.next();
    let said = statement.read::<i64, _>("count").unwrap();

    let mut statement = connection.prepare("SELECT count(writer) AS count FROM zitate WHERE writer = :id").unwrap();
    let _ = statement.bind((":id", user.id as i64));
    let _ = statement.next();
    let wrote = statement.read::<i64, _>("count").unwrap();

    let mut statement = connection.prepare("SELECT count(user) AS count FROM assisted WHERE user = :id").unwrap();
    let _ = statement.bind((":id", user.id as i64));
    let _ = statement.next();
    let assisted = statement.read::<i64, _>("count").unwrap();

    format!(
        "Stats für {}:\nGesagt: {said} ({}%)\nGeschrieben: {wrote} ({}%)\nAssisted: {assisted} ({}%)",
        user.name,
        get_percentage(said),
        get_percentage(wrote),
        get_percentage(assisted)
    )
}

pub fn get_zitate(user: User) -> String {
    let connection = new_connection();
    let mut statement = connection.prepare("
        SELECT z.id as id, z.text as text
        FROM zitate AS z
        JOIN said AS s ON z.id = s.zitat
        WHERE s.user = :user_id
        ORDER BY id
    ").unwrap();
    let _ = statement.bind((":user_id", user.id as i64));

    let zitate: Vec<String> = statement.into_iter().map(|row| {
        let row = row.unwrap();
        format!("{}\nhttps://discord.com/channels/422796692899758091/528316171389239296/{}",
            row.read::<&str, _>("text"),
            row.read::<i64, _>("id")
        )
    }).collect();
    if zitate.is_empty() {
        format!("{} hat noch keine Zitate", user.name)
    } else {
        format!("Zitate von {}:\n\n{}", user.name, zitate.join("\n------------------\n"))
    }
}
