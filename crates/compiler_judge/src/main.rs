use tokio::sync::Semaphore;
use tokio::sync::Mutex;
use tokio::process::Command;
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::env::var;
use std::env::args;
use std::io::prelude::*;
use std::path::PathBuf;
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
    Manual(String),
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
    correct_output: Option<String>,
    user_output_source: UserOutputSource,
    cursor: Arc<Mutex<Cursor>>,
    cursor_pos: usize,
    status: JobStatus,
}

impl Job {
    fn new(name: String, command: String, input: String, correct_output: Option<String>, user_output_source: UserOutputSource, cursor: Arc<Mutex<Cursor>>) -> Self {
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
            JobStatus::Manual(_) => format!("{}MN{}", Cursor::COLOR_BOLD, Cursor::COLOR_RESET),
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

        let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("Cannot spawn task");
        child.stdin.take().unwrap().write_all(self.input.as_bytes()).await.expect("Cannot write to child");

        let mut job_output: String;
        match timeout(Duration::from_secs(*TIMEOUT as u64), child.wait_with_output()).await {
            Ok(output) => {
                let output = output.expect("Error when fetching job output");
                if !output.status.success() {
                    self.status = JobStatus::RuntimeError("Process exited with non-zero code".to_string());
                    self.update_state().await;
                    return;
                }
                job_output = unsafe { String::from_utf8_unchecked(output.stdout) };
            }
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
                        self.status = JobStatus::WrongAnswer(self.correct_output.clone().unwrap_or("".to_string()), "".to_string());
                        self.update_state().await;
                        return;
                    }
                }
            }
        };

        job_output = job_output.trim().to_string();

        self.status = match self.correct_output {
            Some(ref correct_output) => if job_output == *correct_output.trim().to_string() {
                JobStatus::Accepted
            } else {
                JobStatus::WrongAnswer(correct_output.clone(), job_output)
            }
            None => JobStatus::Manual(job_output)
        };
        self.update_state().await;
    }
}

#[derive(serde::Deserialize)]
struct Testcase {
    cmd: String,
    input: Option<String>,
    output: Option<String>,
    answer: Option<String>,
}

#[derive(serde::Deserialize)]
struct TestRoot {
    #[serde(flatten)]
    tests: HashMap<String, Testcase>,
}

#[tokio::main]
async fn main() {
    let cursor = Arc::new(Mutex::new(Cursor::new()));

    let test_path = std::convert::Into::<PathBuf>::into(args().nth(1).unwrap_or(".".to_string()));
    std::env::set_current_dir(test_path).expect("Cannot enter testing directory");


    let config = std::fs::read_to_string("index.toml").expect("Cannot read configuration");
    let test_root: TestRoot = toml::from_str(config.as_str()).expect("Type error in the provided index.toml");

    let mut handles = Vec::new();
    for (name, content) in test_root.tests {
        let mut input = String::new();
        if let Some(ref input_path) = content.input {
            input = std::fs::read_to_string(input_path).unwrap_or_else(|_| panic!("Cannot read input from {}", input_path));
        }

        let output = match content.output {
            Some(output_path) => UserOutputSource::Filename(output_path),
            None => UserOutputSource::Stdout,
        };

        let correct_output = content.answer.as_ref().map(|answer_path| std::fs::read_to_string(answer_path).unwrap_or_else(|_| panic!("Cannot read answer from {}", answer_path)));

        let handle = tokio::spawn({
            let cursor = cursor.clone();
            async move {
                let mut job = Job::new(name, content.cmd, input, correct_output, output, cursor.clone());
                job.spawn().await;
                job
            }
        });
        handles.push(handle);
    }

    let mut collected_jobs = Vec::new();
    for handle in handles {
        if let Ok(job) = handle.await {
            collected_jobs.push(job);
        }
    }

    let mut accepted_cnt = 0;
    let mut accepted = String::new();
    let mut wrong_answer_cnt = 0;
    let mut wrong_answer = String::new();
    let mut time_limit_exceeded_cnt = 0;
    let mut time_limit_exceeded = String::new();
    let mut runtime_error_cnt = 0;
    let mut runtime_error = String::new();
    let mut manual_cnt = 0;
    let mut manual = String::new();
    let tot = collected_jobs.len();
    for job in collected_jobs {
        match job.status {
            JobStatus::Accepted => {
                accepted_cnt += 1;
                accepted += format!("    {}: passed\n", job.name).as_str();
            }
            JobStatus::WrongAnswer(correct, actual) => {
                wrong_answer_cnt += 1;
                wrong_answer += format!("    {}: expected \"{}\", found \"{}\"\n", job.name, correct, actual).as_str();
            }
            JobStatus::TimeLimitExceeded => {
                time_limit_exceeded_cnt += 1;
                time_limit_exceeded += format!("    {}: timeout after {}s\n", job.name, *TIMEOUT).as_str();
            }
            JobStatus::RuntimeError(e) => {
                runtime_error_cnt += 1;
                runtime_error += format!("    {}: runtime error: {}\n", job.name, e).as_str();
            }
            JobStatus::Manual(s) => {
                manual_cnt += 1;
                manual += format!("    {}: result is \"{}\"", job.name, s).as_str();
            }
            JobStatus::Tbd => ()
        }
    }

    let _ = cursor.lock().await.new_line();

    println!("\nSummary:\n");
    if accepted_cnt != 0 {
        println!("  Accepted ({}/{}):\n{}", accepted_cnt, tot, accepted);
    }
    if time_limit_exceeded_cnt != 0 {
        println!("  Time Limit Exceeded ({}/{}):\n{}", time_limit_exceeded_cnt, tot, time_limit_exceeded);
    }
    if runtime_error_cnt != 0 {
        println!("  Runtime Error ({}/{}):\n{}", runtime_error_cnt, tot, runtime_error);
    }
    if wrong_answer_cnt != 0 {
        println!("  Wrong Answer ({}/{}):\n{}", wrong_answer_cnt, tot, wrong_answer);
    }
    if manual_cnt != 0 {
        println!("  Manual Check ({}/{}):\n{}", manual_cnt, tot, manual);
    }
}
