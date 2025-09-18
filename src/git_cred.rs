use anyhow::{Result, anyhow};
use derive_more::Debug;
use serde::Deserialize;
use std::process::{Command, Stdio};
use which::which;

#[derive(Deserialize, Debug)]
pub struct GitCredential {
    protocol: String,
    host: String,
    username: String,
    #[debug(ignore)]
    password: String,
}

pub async fn process_credential(cred: GitCredential) -> Result<()> {
    println!("Received credential: {:?}", cred);

    if cred.host == "github.com" && cred.protocol == "https" {
        if which("gh").is_ok() {
            println!("Using gh auth login --with-token to update credentials");
            let mut child = Command::new("gh")
                .args(&["auth", "login", "--with-token"])
                .stdin(Stdio::piped())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn()?;

            {
                let stdin = child
                    .stdin
                    .as_mut()
                    .ok_or_else(|| anyhow!("Failed to open stdin"))?;
                use std::io::Write;
                stdin.write_all(cred.password.as_bytes())?;
            }

            let status = child.wait()?;
            if !status.success() {
                println!("gh auth login failed");
            }
        } else {
            println!("gh command not found");
        }
    }

    let input = format!(
        "protocol={}\nhost={}\nusername={}\npassword={}\n\n",
        cred.protocol, cred.host, cred.username, cred.password
    );

    let mut child = Command::new("git")
        .args(&["credential", "approve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow!("Failed to open stdin"))?;
        use std::io::Write;
        stdin.write_all(input.as_bytes())?;
    }

    let status = child.wait()?;
    if !status.success() {
        eprintln!("git credential approve failed");
    }

    Ok(())
}
