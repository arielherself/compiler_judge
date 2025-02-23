use tokio::sync::Semaphore;
use tokio::sync::Mutex;
use tokio::process::Command;
use lazy_static::lazy_static;
use std::env::var;
use std::io::prelude::*;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::time::timeout;
use std::time::Duration;

lazy_static! {
    static ref TIMEOUT: usize = var("COMPILER_JUDGE_TIMEOUT").unwrap_or("2".to_string()).parse().unwrap();
    static ref NJOBS: usize = var("COMPILER_JUDGE_NJOBS").unwrap_or("4".to_string()).parse().unwrap();
    static ref JOB_PERMITS: Semaphore = Semaphore::const_new(*NJOBS);
}

struct Cursor {
    pos: usize,
    tot: usize,
}

impl Cursor {
    const ESC: &'static str = "\x1b";
    const COLOR_BOLD: &'static str = "\x1b[1m";
    const COLOR_GREEN: &'static str = "\x1b[32m";
    const COLOR_RED: &'static str = "\x1b[31m";
    const COLOR_MAGENTA: &'static str = "\x1b[35m";
    const COLOR_CYAN: &'static str = "\x1b[36m";
    const COLOR_RESET: &'static str = "\x1b[0m";
    fn new() -> Self {
        Self {
            pos: 0,
            tot: 1,
        }
    }
    fn flush(&self) {
        std::io::stdout().flush().expect("Cannot flush stdout");
    }
    fn move_to(&mut self, new_pos: usize) -> usize {
        assert!(new_pos < self.tot, "Invalid cursor position");
        match new_pos.cmp(&self.pos) {
            std::cmp::Ordering::Less => {
                print!("{}[{}F", Self::ESC, self.pos - new_pos);
            }
            std::cmp::Ordering::Greater => {
                print!("{}[{}E", Self::ESC, new_pos - self.pos);
            }
            _ => {}
        }
        self.pos = new_pos;
        self.flush();
        self.pos
    }
    fn new_line(&mut self) -> usize {
        self.move_to(self.tot - 1);
        println!();
        self.flush();
        self.pos = self.tot;
        self.tot += 1;
        self.pos
    }
    fn write_line(&mut self, pos: usize, content: String) -> usize {
        self.move_to(pos);
        print!("\r{}[0K{}", Self::ESC, content);
        self.flush();
        self.pos
    }
}

enum JobStatus {
    Accepted,
    WrongAnswer(String, String),
    TimeLimitExceeded,
    RuntimeError(String),
    Tbd,
}

enum UserOutputSource {
    Stdout,
    Filename(String),
}

struct Job {
    name: String,
    command: String,
    input: String,
    correct_output: String,
    user_output_source: UserOutputSource,
    cursor: Arc<Mutex<Cursor>>,
    cursor_pos: usize,
    status: JobStatus,
}

impl Job {
    fn new(name: String, command: String, input: String, correct_output: String, user_output_source: UserOutputSource, cursor: Arc<Mutex<Cursor>>) -> Self {
        Self {
            name,
            command,
            input,
            correct_output,
            user_output_source,
            cursor,
            cursor_pos: 0,
            status: JobStatus::Tbd,
        }
    }

    async fn update_state(&mut self) {
        let state_str = match self.status {
            JobStatus::Tbd => "..".to_string(),
            JobStatus::Accepted => format!("{}{}AC{}", Cursor::COLOR_BOLD, Cursor::COLOR_GREEN, Cursor::COLOR_RESET),
            JobStatus::WrongAnswer(_, _) => format!("{}{}WA{}", Cursor::COLOR_BOLD, Cursor::COLOR_RED, Cursor::COLOR_RESET),
            JobStatus::TimeLimitExceeded => format!("{}{}TL{}", Cursor::COLOR_BOLD, Cursor::COLOR_MAGENTA, Cursor::COLOR_RESET),
            JobStatus::RuntimeError(_) => format!("{}{}RE{}", Cursor::COLOR_BOLD, Cursor::COLOR_CYAN, Cursor::COLOR_RESET),
        };
        self.cursor.lock().await.write_line(self.cursor_pos, format!("Test: {}    [ {} ]", self.name, state_str));
    }

    async fn spawn(&mut self) {
        let _permit = JOB_PERMITS.acquire().await.expect("Job error when acquiring a permit");
        {
            let mut guard = self.cursor.lock().await;
            self.cursor_pos = guard.new_line();
            std::mem::drop(guard);
        }
        self.update_state().await;

        let mut cmdlist = self.command.split(' ');
        let mut command = Command::new(cmdlist.next().expect("Job is empty"));
        for item in cmdlist {
            command.arg(item);
        }

        let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("Cannot spawn task");
        child.stdin.take().unwrap().write_all(self.input.as_bytes()).await.expect("Cannot write to child");

        let mut job_output: String;
        match timeout(Duration::from_secs(*TIMEOUT as u64), child.wait_with_output()).await {
            Ok(output) => job_output = unsafe { String::from_utf8_unchecked(output.expect("Error when fetching job output").stdout) },
            Err(_) => {
                self.status = JobStatus::TimeLimitExceeded;
                self.update_state().await;
                return;
            }
        }

        match self.user_output_source {
            UserOutputSource::Stdout => (),
            UserOutputSource::Filename(ref filename) => {
                match std::fs::read_to_string(filename) {
                    Ok(s) => job_output = s,
                    Err(_) => {
                        self.status = JobStatus::WrongAnswer(self.correct_output.clone(), "".to_string());
                        self.update_state().await;
                        return;
                    }
                }
            }
        };

        job_output = job_output.trim().to_string();
        self.correct_output = self.correct_output.trim().to_string();

        self.status = if job_output == self.correct_output {
            JobStatus::Accepted
        } else {
            JobStatus::WrongAnswer(self.correct_output.clone(), job_output)
        };
        self.update_state().await;
    }
}

#[tokio::main]
async fn main() {
    let cursor = Arc::new(Mutex::new(Cursor::new()));
    let mut job1 = Job::new(
        "job1".to_string(),
        "sleep 2".to_string(),
        "".to_string(),
        "hello".to_string(),
        UserOutputSource::Stdout, cursor.clone()
    );
    let mut job2 = Job::new(
        "job2".to_string(),
        "sleep 1".to_string(),
        "".to_string(),
        "".to_string(),
        UserOutputSource::Stdout, cursor.clone()
    );
    let mut job3 = Job::new(
        "job3".to_string(),
        "echo world".to_string(),
        "".to_string(),
        "world".to_string(),
        UserOutputSource::Stdout, cursor.clone()
    );
    let mut job4 = Job::new(
        "job4".to_string(),
        "sleep 3".to_string(),
        "".to_string(),
        "".to_string(),
        UserOutputSource::Stdout, cursor.clone()
    );
    let mut job5 = Job::new(
        "job5".to_string(),
        "sleep 1".to_string(),
        "".to_string(),
        "".to_string(),
        UserOutputSource::Stdout, cursor.clone()
    );
    let mut job6 = Job::new(
        "job6".to_string(),
        "sleep 1".to_string(),
        "".to_string(),
        "".to_string(),
        UserOutputSource::Stdout, cursor.clone()
    );
    let job1_future = job1.spawn();
    let job2_future = job2.spawn();
    let job3_future = job3.spawn();
    let job4_future = job4.spawn();
    let job5_future = job5.spawn();
    let job6_future = job6.spawn();
    tokio::join!(job1_future, job2_future, job3_future, job4_future, job5_future, job6_future);

    if let JobStatus::WrongAnswer(correct, actual) = job1.status {
        let ln = cursor.lock().await.new_line();
        cursor.lock().await.write_line(ln, format!("Wrong answer: expected \"{}\", found \"{}\"", correct, actual));
    };

    let _ = cursor.lock().await.new_line();
}
