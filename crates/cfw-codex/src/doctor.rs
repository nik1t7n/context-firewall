use std::process::Command;

#[derive(Debug, Clone)]
pub struct CodexDoctor {
    pub codex_found: bool,
    pub version: Option<String>,
    pub hook_replacement_verified: bool,
}

pub fn check() -> CodexDoctor {
    let output = Command::new("codex").arg("--version").output();
    match output {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            CodexDoctor {
                codex_found: true,
                version: Some(version),
                hook_replacement_verified: false,
            }
        }
        _ => CodexDoctor {
            codex_found: false,
            version: None,
            hook_replacement_verified: false,
        },
    }
}
