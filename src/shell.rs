use crate::send;
use derive_more::Debug;
use serde::Deserialize;
use std::process::{Command, Stdio};

#[derive(Deserialize, Debug)]
pub struct ShellParam {
    command: Vec<String>,
    env: Option<Vec<(String, String)>>,
}

pub fn process_shell(client_name: String, shell_param: ShellParam) {
    println!("Received shell command: {:?}", shell_param);

    if shell_param.command.is_empty() {
        println!("No command provided");
        return;
    }

    let mut cmd = Command::new(&shell_param.command[0]);
    if shell_param.command.len() > 1 {
        cmd.args(&shell_param.command[1..]);
    }

    if let Some(env_vars) = shell_param.env {
        for (key, value) in env_vars {
            cmd.env(key, value);
        }
    }

    cmd.stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .stdin(Stdio::inherit());

    match cmd.spawn() {
        Ok(mut child) => match child.wait() {
            Ok(status) => {
                if status.success() {
                    println!("Command executed successfully");
                    send::send_log_local(
                        client_name,
                        "[shell] Command executed successfully".into(),
                    );
                } else {
                    eprintln!("Command exited with status: {}", status);
                    send::send_log_local(
                        client_name,
                        format!("[shell] Command exited with status: {}", status),
                    );
                }
            }
            Err(e) => {
                eprintln!("Failed to wait on child process: {}", e);
                send::send_log_local(
                    client_name,
                    format!("[shell] Failed to wait on child process: {}", e),
                );
            }
        },
        Err(e) => {
            eprintln!("Failed to execute command: {}", e);
            send::send_log_local(
                client_name,
                format!("[shell] Failed to execute command: {}", e),
            );
        }
    }
}
