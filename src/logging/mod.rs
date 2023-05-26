use std::{fs::{OpenOptions, self}, io::Write, time::{SystemTime, UNIX_EPOCH}};
use chrono::Local;
use lazy_static::lazy_static;

lazy_static! {
    static ref START_TIME: u128 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
}

pub fn log(message: &str, log_level: &str) {
    let print_string = format!("[{}] [{log_level}] {message}", get_date_string());
    println!("{print_string}");
    log_to_file(print_string);
}

fn get_log_file_path() -> String {
    format!("logs/{}.log", *START_TIME)
}

pub fn log_to_file(print_string: String) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(get_log_file_path())
        .expect("Error opening log file");
    file.write_all(print_string.as_bytes()).expect("Error writing to log file");
}

pub fn get_date_string() -> String {
    let now = Local::now();
    now.format("%d.%m.%Y %H:%M:%S").to_string()
}

pub fn delete() {
    fs::remove_file(get_log_file_path()).expect("Error deleting log file");
}
