use std::fs::File;
use std::io::Write;
use std::sync::{LazyLock, Mutex};

pub struct Logger {
    file: File,
}

impl Logger {
    pub fn new(file_path: &str) -> Self {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(file_path)
            .unwrap();
        Logger { file }
    }

    pub fn log(&mut self, message: &str) {
        let _ = writeln!(&self.file, "{}", message);
    }
}

pub static LOGGER: LazyLock<Mutex<Logger>> = LazyLock::new(|| Mutex::new(Logger::new("log.txt")));

pub fn log(val: &str) {
    LOGGER.lock().unwrap().log(val);
}
