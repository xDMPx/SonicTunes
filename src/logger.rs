use std::io::Write;

#[derive(Clone)]
pub struct LogSender {
    sender: crossbeam::channel::Sender<LogMessage>,
}

#[derive(Debug)]
pub enum LogMessage {
    Message(String),
    Quit,
}

impl LogSender {
    pub fn new(sender: crossbeam::channel::Sender<LogMessage>) -> Self {
        Self { sender }
    }

    pub fn send_log_message(&self, msg: String) {
        let send = &self.sender;
        send.send(LogMessage::Message(msg)).unwrap();
    }

    pub fn send_quit_signal(&self) {
        let send = &self.sender;
        send.send(LogMessage::Quit).unwrap();
    }
}

pub struct Logger {
    logger_signal_recv: crossbeam::channel::Receiver<LogMessage>,
    logger_signal_send: crossbeam::channel::Sender<LogMessage>,
}

impl Logger {
    pub fn new() -> Self {
        let (s, r) = crossbeam::channel::unbounded();

        Self {
            logger_signal_recv: r,
            logger_signal_send: s,
        }
    }

    pub fn get_signal_send(&self) -> crossbeam::channel::Sender<LogMessage> {
        return self.logger_signal_send.clone();
    }

    pub fn log(&self) {
        loop {
            let recv = &self.logger_signal_recv;
            if let Ok(signal) = recv.recv() {
                match signal {
                    LogMessage::Message(msg) => self.log_to_file(&msg),
                    LogMessage::Quit => break,
                }
            }
        }
    }

    fn log_to_file(&self, message: &str) {
        let mut log_file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open("debug.log")
            .unwrap();

        writeln!(log_file, "{}", message).unwrap();
    }

    pub fn flush(&self) {
        let recv = &self.logger_signal_recv;
        for _i in 0..recv.len() {
            let signal = recv.recv().unwrap();
            if let LogMessage::Message(msg) = signal {
                self.log_to_file(&msg)
            }
        }
    }
}

impl log::Log for LogSender {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.target().starts_with(env!("CARGO_PKG_NAME"))
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let timestamp = chrono::Utc::now();

            let module_path = record.module_path().unwrap_or("");
            let msg = format!(
                "{:?} [{}] {}: {}",
                timestamp,
                record.level(),
                module_path,
                record.args()
            );
            self.send_log_message(msg.to_owned());
        }
    }

    fn flush(&self) {}
}
