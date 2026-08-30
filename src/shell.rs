use crate::send;
use derive_more::Debug;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

#[derive(Deserialize, Debug)]
pub struct ShellParam {
    command: Vec<String>,
    env: Option<Vec<(String, String)>>,
    /// Optional caller-supplied id, echoed back so the caller can correlate
    /// the async result with the request it sent.
    id: Option<String>,
}

#[derive(Serialize)]
struct ShellResult<'a> {
    id: Option<&'a str>,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    error: Option<String>,
}

pub fn process_shell(client_name: String, shell_param: ShellParam) {
    println!("Received shell command: {:?}", shell_param);
    send::send_log_local(
        client_name.clone(),
        format!("[shell] Received shell command: {:?}", shell_param),
    );

    if shell_param.command.is_empty() {
        println!("No command provided");
        return;
    }

    // Run on a task and capture output so the dispatcher isn't blocked and the
    // caller can pick up the full stdout/stderr asynchronously.
    tokio::spawn(async move {
        run_and_report(client_name, shell_param).await;
    });
}

async fn run_and_report(client_name: String, shell_param: ShellParam) {
    let id = shell_param.id;

    let mut cmd = Command::new(&shell_param.command[0]);
    if shell_param.command.len() > 1 {
        cmd.args(&shell_param.command[1..]);
    }
    if let Some(env_vars) = shell_param.env {
        for (key, value) in env_vars {
            cmd.env(key, value);
        }
    }

    let result = match cmd.output().await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            println!("Command finished (exit {:?})", output.status.code());
            ShellResult {
                id: id.as_deref(),
                exit_code: output.status.code(),
                stdout,
                stderr,
                error: None,
            }
        }
        Err(e) => {
            eprintln!("Failed to execute command: {}", e);
            ShellResult {
                id: id.as_deref(),
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(e.to_string()),
            }
        }
    };

    let payload = serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(
            "{{\"id\":null,\"error\":\"failed to serialize shell result: {}\"}}",
            e
        )
    });
    send::send_shell_result(client_name, payload);
}
