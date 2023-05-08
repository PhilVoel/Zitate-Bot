use std::{fs::OpenOptions, io::Write};
use chrono::Local;
use crate::START_TIME;

pub fn log(message: &str, r#type: &str) {
    let print_string = format!("[{}] [{}] {}", get_date_string(), r#type, message);
    println!("{}", print_string);
    log_to_file(print_string);
}

pub fn get_log_file_path() -> String {
    let file_path;
    unsafe {
        file_path = format!("logs/{}.log", START_TIME);
    }
    file_path
}

pub fn log_to_file(print_string: String) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(get_log_file_path())
        .unwrap();
    file.write_all(print_string.as_bytes()).unwrap();
}

pub fn get_date_string() -> String {
    let now = Local::now();
    now.format("%d.%m.%Y %H:%M:%S").to_string()
}
