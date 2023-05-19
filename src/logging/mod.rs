use std::{fs::{OpenOptions, self}, io::Write};
use chrono::Local;
pub static mut START_TIME: u128 = 0;

pub fn log(message: &str, log_level: &str) {
    let print_string = format!("[{}] [{log_level}] {message}", get_date_string());
    println!("{print_string}");
    log_to_file(print_string);
}

fn get_log_file_path() -> String {
    let file_path;
    unsafe {
        file_path = format!("logs/{START_TIME}.log");
    }
    file_path
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
