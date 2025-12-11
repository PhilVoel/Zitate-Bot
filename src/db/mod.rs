use std::sync::RwLock;

use pml::PmlStruct;
use serenity::model::{prelude::Message, id::UserId as SerenityUserId};

use crate::{logging::log, QAType, RankingType, OVERALL_ZITATE_COUNT};

pub mod user;
pub use user::User;

static DB_FILE: RwLock<String> = RwLock::new(String::new());

pub fn new_connection() -> sqlite::Connection {
    sqlite::open(&*DB_FILE.read().unwrap()).expect("Failed to open DB")
}

pub fn add_qa(r#type: QAType, user: User, id: u64) -> String {
    let connection = new_connection();

    let mut statement = connection.prepare("SELECT * FROM said WHERE zitat=:zitat AND user=:user").unwrap();
    statement.bind((":zitat", id as i64)).unwrap();
    statement.bind((":user", user.id as i64)).unwrap();
    let already_said = statement.into_iter().next().is_some();

    let mut statement = connection.prepare("SELECT * FROM assisted WHERE zitat=:zitat AND user=:user").unwrap();
    statement.bind((":zitat", id as i64)).unwrap();
    statement.bind((":user", user.id as i64)).unwrap();
    let already_assisted = statement.into_iter().next().is_some();

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
    let mut statement = connection.prepare(format!("INSERT INTO {table_name}(zitat, user) VALUES(:zitat, :user)")).unwrap();
    let _ = statement.bind((":zitat", id as i64));
    let _ = statement.bind((":user", user.id as i64));
    let _ = statement.next();

    log(&format!("Added {} to {table_name} of Zitat with ID {id} in DB", user.name), "INFO");
    format!("{} erfolgreich hinzugefügt.", user.name)
}

pub fn get_ranking(r#type: RankingType) -> String {
    let connection = new_connection();

    let (type_de, statement) = match r#type {
        RankingType::Said => {
            let statement = connection.prepare(format!("SELECT COUNT(t.user) as count, users.name
                FROM
                users
                LEFT JOIN said AS t
                ON users.id = t.user
                GROUP BY users.id
                HAVING count > 0
                ORDER BY count DESC
                ")).unwrap();
            ("gesprochene", statement)
        },
        RankingType::Wrote => {
            let statement = connection.prepare(format!("SELECT COUNT(t.writer) as count, users.name
                FROM
                users
                LEFT JOIN zitate AS t
                ON users.id = t.writer
                GROUP BY users.id
                HAVING count > 0
                ORDER BY count DESC
                ")).unwrap();
            ("geschriebene", statement)
        },
        RankingType::Assisted => {
            let statement = connection.prepare(format!("SELECT COUNT(t.user) as count, users.name
                FROM
                users
                LEFT JOIN assisted AS t
                ON users.id = t.user
                GROUP BY users.id
                HAVING count > 0
                ORDER BY count DESC
                ")).unwrap();
            ("assistierte", statement)
        },
    };

    format!(
        "Ranking {type_de} Zitate:\n{}",
        statement.into_iter()
            .enumerate()
            .map(|(i, r)| {
                let row = r.unwrap();
                format!(
                    "{:02}.: {}: {} ({}%)",
                    i + 1,
                    row.read::<&str, _>("name"),
                    row.read::<i64, _>("count"),
                    get_percentage(row.read::<i64, _>("count"))
                )
            })
            .collect::<Vec<String>>()
            .join("\n")
    )
}

pub fn init(config: &PmlStruct) {
    *DB_FILE.write().unwrap() = config.get::<String>("dbFile").expect("dbFile value not found in config file");
    let connection = new_connection();
    let _ = connection.execute("CREATE TABLE IF NOT EXISTS users(
        id INTEGER PRIMARY KEY,
        name TEXT UNIQUE NOT NULL
    )");
    let _ = connection.execute("CREATE TABLE IF NOT EXISTS other_ids(
        secondary_id INTEGER PRIMARY KEY,
        main_id INTEGER NOT NULL,
        FOREIGN KEY(main_id) REFERENCES users(id) ON DELETE RESTRICT
    )");
    let _ = connection.execute("CREATE TABLE IF NOT EXISTS zitate(
        id INTEGER PRIMARY KEY,
        text TEXT NOT NULL,
        time STRING NOT NULL,
        writer INTEGER NOT NULL,
        FOREIGN KEY(writer) REFERENCES users(id)
    )");
    let _ = connection.execute("CREATE TABLE IF NOT EXISTS said(
        zitat INTEGER,
        user INTEGER,
        PRIMARY KEY(zitat, user),
        FOREIGN KEY(zitat) REFERENCES zitate(id) ON DELETE CASCADE,
        FOREIGN KEY(user) REFERENCES users(id)
    )");
    let _ = connection.execute("CREATE TABLE IF NOT EXISTS assisted(
        zitat INTEGER,
        user INTEGER,
        PRIMARY KEY(zitat, user),
        FOREIGN KEY(zitat) REFERENCES zitate(id) ON DELETE CASCADE,
        FOREIGN KEY(user) REFERENCES users(id)
    )");
    log("Set up database", "INFO");
}

fn get_percentage(count: i64) -> f32 {
    let total;
    unsafe {
        total = OVERALL_ZITATE_COUNT;
    }
    (count as f32 * 10_000.0 / total as f32).round() / 100.0
}

pub fn insert_zitat(zitat_msg: &Message) {
    let SerenityUserId(author_id) = zitat_msg.author.id;
    let msg_id = zitat_msg.id.as_u64();
    let author = match user::get(&author_id) {
        Some(user_data) => user_data,
        None => {
            log("Author not found in DB", "WARN");
            user::add(author_id, &zitat_msg.author.name);
            User::new(author_id, zitat_msg.author.name.clone()) 
        }
    };

    let connection = new_connection();
    let mut statement = connection.prepare("INSERT INTO zitate(id, text, time, writer) VALUES(:id, :text, :time, :writer)").unwrap();
    statement.bind((":id", *msg_id as i64)).unwrap();
    statement.bind((":text", zitat_msg.content.trim())).unwrap();
    statement.bind((":time", zitat_msg.timestamp.to_rfc3339().as_str())).unwrap();
    statement.bind((":writer", author.id as i64)).unwrap();
    let _ = statement.next();
    log(&format!("Zitat with ID {msg_id} successfully inserted into DB"), "INFO");
}

pub fn delete_zitat(id: u64) {
    let connection = new_connection();
    let mut statement = connection.prepare("SELECT z.text AS content, datetime(z.time, 'unixepoch') AS timestamp, u.name AS author_name
        FROM zitate AS z
        JOIN users AS u ON z.writer = u.id
        WHERE z.id = :id
    ").unwrap();
    statement.bind((":id", id as i64)).unwrap();
    statement.next().unwrap();

    log(&format!("Content: {}", statement.read::<String, _>("content").unwrap()), "INFO");
    log(&format!("Author:  {}", statement.read::<String, _>("author_name").unwrap()), "INFO");
    log(&format!("Date:    {}", statement.read::<String, _>("timestamp").unwrap()), "INFO");

    connection.execute(format!("DELETE FROM zitate WHERE id = {id}")).unwrap();
    log("Deleted from DB", "INFO");
}
