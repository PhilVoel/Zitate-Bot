use std::{fs::{OpenOptions, self}, io::Write, time::{SystemTime, UNIX_EPOCH}, sync::OnceLock};
use chrono::Local;

static START_TIME: OnceLock<u128> = OnceLock::new();

pub fn log(message: &str, log_level: &str) {
    let print_string = format!("[{}] [{log_level}] {message}", get_date_string());
    println!("{print_string}");
    log_to_file(print_string);
}

fn get_log_file_path() -> String {
    let start_time = START_TIME.get_or_init(|| SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis());
    format!("logs/{}.log", start_time)
}

pub fn log_to_file(print_string: String) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(get_log_file_path())
        .expect("Error opening log file");
    file.write_all((print_string+"\n").as_bytes()).expect("Error writing to log file");
}

pub fn get_date_string() -> String {
    let now = Local::now();
    now.format("%d.%m.%Y %H:%M:%S").to_string()
}

pub fn delete() {
    fs::remove_file(get_log_file_path()).expect("Error deleting log file");
}
