// TODO: fix finished job notifs so that they don't get put ont he line that is currently being typed

extern crate dirs;
extern crate nix;

use std::path::{Path};
use std::{env, thread};
use std::fs::File;
use std::io::{stdin, stdout, Write, Read};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use ansi_term::{Color, Style};

// Global Constants
const MAX_HISTORY:usize = 10;
const MAX_JOBS:usize = 255;

struct HistoryStruct {
    history: [String; MAX_HISTORY],
    history_count: usize,
}

impl HistoryStruct {
    fn new() -> Self {
        HistoryStruct {
            history: std::array::from_fn(|_| String::new()),
            history_count: 0,
        }
    }

    fn add_to_history(&mut self, line: &String) {
        if self.history_count == MAX_HISTORY {

        } else {
            self.history[self.history_count] = line.clone();
            self.increment_history_count();
        }
    }
    fn increment_history_count(&mut self) {
        self.history_count += 1;
    }

    fn print_history(&self) {
        for i in 0..self.history_count {
            println!("{} {}", i+1, self.history[i]);
        }
    }
}

struct JobStruct {
    status: bool,
    command: String,
    id: u32,
    process: Child,
}

impl JobStruct {
    fn new(command: String, child: Child, current_job_id: u32) -> Self {
        JobStruct {
            id: current_job_id,
            command,
            status: true,
            process: child,
        }
    }

    // fn add_job(&mut self, job: String, child: Child) {
    //     self.jobs.push(job);
    //     self.job_status.push(true);
    //     self.job_ids.push(self.current_id);
    //     self.job_processes.push(child);
    //     self.current_id+=1;
    // }

    fn get_status(status: bool) -> String {
        if status {
            String::from("running")
        } else {
            String::from("stopped")
        }
    }
}

// TODO: implement this somehow
struct SettingsStruct {
    font: String,
    font_size: u32,
}

impl SettingsStruct {
    fn new() -> Self {
        SettingsStruct {
            font: String::new(),
            font_size: SettingsStruct::get_current_font_size(),
        }
    }

    fn get_current_font_size() -> u32 {
        // match env::consts::OS {
        //     "linux" => println!("Running on Linux"),
        //     "macos" => println!("Running on macOS"),
        //     "windows" => println!("Running on Windows"),
        //     _ => println!("Running on an unknown OS"),
        // }
        return 0;
    }
}

struct ShellStruct {
    history_struct: HistoryStruct,
    jobs: Vec<JobStruct>,
    settings_struct: SettingsStruct,
    current_job_id: u32,
}

impl ShellStruct {
    fn new() -> Self {
        ShellStruct {
            history_struct: HistoryStruct::new(),
            jobs: Vec::new(),
            settings_struct: SettingsStruct::new(),
            current_job_id: 0,
        }
    }

    fn add_job(&mut self, command: String, child: Child) {
        self.jobs.push(JobStruct::new(command, child, self.current_job_id));
        self.current_job_id += 1;
    }

    fn print_jobs(&self) {
        for (i, job) in self.jobs.iter().enumerate() {
            let status: String = JobStruct::get_status(job.status);
            println!("[{}]  {}", job.id, status);
        }
    }

    fn update_jobs(&mut self) {
        //self.jobs.retain_mut(|job| !matches!(job.process.try_wait(), Ok(Some(_))));
        self.jobs.retain_mut(
            |job|
                match job.process.try_wait() {
                    Ok(Some(_)) => {
                        println!("Done!");
                        false
                    },
                    _ => true,
                }
        );
    }
}


fn main() {
    println!("Shell!");

    let mut shell_struct = Arc::new(Mutex::new(ShellStruct::new()));

    let home_dir = dirs::home_dir().unwrap();
    let _=env::set_current_dir(&home_dir.as_path());

    // check and update jobs in its own thread
    let thread_shell_struct = Arc::clone(&shell_struct);
    let jobs_thread = thread::spawn(move || {
        loop {
            let mut lock = thread_shell_struct.lock().unwrap();
            lock.update_jobs();
        }

    });

    loop {
        print_pwd();

        // get and parse input
        let mut input = String::new();
        let _=stdout().flush();
        stdin().read_line(&mut input).unwrap();
        input.pop();  // remove \n

        if input == "" { continue; }

        shell_struct.lock().unwrap().history_struct.add_to_history(&input);

        let mut tokens: Vec<&str> = input.split(" ").collect();
        tokens.retain(|&char| char != "");  // get rid of extra spaces

        while !tokens.is_empty() {  // loop while && or || still exists
            let (mut tokens_part, separator) = split_input(&mut tokens);

            let ampersand: bool;
            if tokens_part[tokens_part.len()-1] == "&" {
                ampersand = true;
                tokens_part.remove(tokens_part.len()-1);
            } else {
                ampersand = false;
            }

            let command: String = tokens_part[0].to_lowercase();
            tokens_part.remove(0);  // remove the command from the tokens vec

            // run command and check if it exists, continue if true
            let result: bool = execmd(&shell_struct.lock().unwrap(), &command, &tokens_part);
            if result { continue }

            // fork and run
            let mut cmd = Command::new(command.as_str()).args(&tokens_part).spawn();
            match cmd {
                Ok(ref mut child) => {
                    if !ampersand {
                        match child.wait() {
                            Ok(exit_status) => {
                                if separator.is_some() {
                                    let separator_copy = separator.unwrap().clone();
                                    if separator_copy == "||" {
                                        if exit_status.success() {
                                            break;
                                        }
                                    } else if separator_copy == "&&" {
                                        if exit_status.success() {
                                            continue;
                                        } else {
                                            break;
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                println!("Some child error")
                            }
                        }
                    } else {
                        tokens_part.insert(0, command.as_str());
                        shell_struct.lock().unwrap().add_job(
                            tokens_part.join(" "), cmd.unwrap()
                        );
                    }

                },
                Err(_) => {
                    println!("Command {command} not recognized");

                    if separator.is_some() && separator.unwrap() == "&&" {
                        break;
                    }
                }
            }
        }
    }
}

/**
    Checks the command and calls the proper function
    Returns true if command is recognized, otherwise false
 */
fn execmd(shell_struct: &ShellStruct, command: &String, args: &Vec<&str>) -> bool {
    let command_str: &str = command.as_str();
    match command_str {
        "cd"      => cd(args),
        "cat"     => cat(args),
        "history" => shell_struct.history_struct.print_history(),
        "jobs"    => shell_struct.print_jobs(),
        _         => return false,
    };
    return true
}

fn get_pwd() -> String {
    env::current_dir().unwrap().into_os_string().into_string().unwrap()
}
fn print_pwd() {
    let pwd = get_pwd();
    let last_index = pwd.rfind('/').unwrap();
    print!("{} ", &pwd[last_index+1..]);
}

fn cd(args: &Vec<&str>) {
    if args.is_empty() {
        let home_dir = dirs::home_dir().unwrap();
        let _=env::set_current_dir(&home_dir.as_path());
        return
    } else if args.len() > 1 {
        println!("cd: too many arguments");
        return
    }
    let dir_path: &str = args[0];
    let path = Path::new(&dir_path);
    let dir_set_result = env::set_current_dir(&path);

    match dir_set_result {
        Ok(dir) => dir,
        Err(err) => {
            println!("cd: no such file or directory");
            return
        },
    };
}

/**
 * Exists because normal cat does not make a new line if new line is not included at the end of the file
 */
fn cat(args: &Vec<&str>) {
    // let pwd = get_pwd();
    let path = args[0];
    if let Ok(mut file) = File::open(path) {

        let mut buffer: [u8; 1] = [0; 1];
        loop {
            match file.read(&mut buffer) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let ascii_char = buffer[0] as char;
                    print!("{ascii_char}");
                },
                Err(e) => println!("{:?}", e),
            }
        }

        let percent_color: Style = Color::Black.on(Color::White);
        if buffer[0] as char != '\n' {
            println!("{}", percent_color.paint("%"))  // put a % if no new line
        }

    } else {
        println!("cat: No such file or directory: {path}");
    }
}

fn split_input<'a>(tokens: &'a mut Vec<&str>) -> (Vec<&'a str>, Option<String>) {
    let mut part = Vec::new();
    let mut part_index = 0;
    let mut separator: Option<String> = None;

    for token in &mut *tokens {
        part_index+=1;
        if *token == "&&" {
            separator = Some("&&".to_string());
            break;
        } else if *token == "||" {
            separator = Some("||".to_string());
            break;
        }
        part.push(*token);
    }
    for i in 0..part_index {
        tokens.remove(0);
    }
    return (part, separator);
}