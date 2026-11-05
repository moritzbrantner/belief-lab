use crate::error::SemifSetupError;
use std::process::{Command, Stdio};

pub(crate) fn local_command_output(command: &mut Command) -> Option<String> {
    let output = command.output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn run_command(command: &mut Command, purpose: &str) -> Result<(), SemifSetupError> {
    let status = command
        .stdout(Stdio::from(std::io::stderr()))
        .status()
        .map_err(|error| SemifSetupError::Command {
            context: purpose.into(),
            message: error.to_string(),
        })?;
    if !status.success() {
        return Err(SemifSetupError::Command {
            context: purpose.into(),
            message: format!("process exited with {status}"),
        });
    }
    Ok(())
}

pub(crate) fn command_output(
    command: &mut Command,
    purpose: &str,
) -> Result<String, SemifSetupError> {
    let output = command.output().map_err(|error| SemifSetupError::Command {
        context: purpose.into(),
        message: error.to_string(),
    })?;
    if !output.status.success() {
        return Err(SemifSetupError::Command {
            context: purpose.into(),
            message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
