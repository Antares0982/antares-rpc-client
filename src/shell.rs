use derive_more::Debug;
use serde::Deserialize;
use std::process::{Command, Stdio};

#[derive(Deserialize, Debug)]
pub struct ShellParam {
    command: Vec<String>,
    env: Vec<(String, String)>,
}

pub fn process_shell(shell_param: ShellParam) {
    println!("Received shell command: {:?}", shell_param);

    if shell_param.command.is_empty() {
        println!("No command provided");
        return;
    }

    let mut cmd = Command::new(&shell_param.command[0]);
    if shell_param.command.len() > 1 {
        cmd.args(&shell_param.command[1..]);
    }

    for (key, value) in shell_param.env {
        cmd.env(key, value);
    }

    cmd.stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit());

    match cmd.spawn() {
        Ok(mut child) => match child.wait() {
            Ok(status) => {
                if status.success() {
                    println!("Command executed successfully");
                } else {
                    eprintln!("Command exited with status: {}", status);
                }
            }
            Err(e) => {
                eprintln!("Failed to wait on child process: {}", e);
            }
        },
        Err(e) => {
            eprintln!("Failed to execute command: {}", e);
        }
    }
}
