use crate::error::SemifSetupError;
use crate::model::{SemifModelPin, SemifModelTier};
use crate::process::run_command;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn model_path(tier: SemifModelTier, directory: &Path) -> PathBuf {
    directory.join(tier.pin().gguf_file)
}

pub fn model_is_ready(tier: SemifModelTier, directory: &Path) -> Result<bool, SemifSetupError> {
    let target = model_path(tier, directory);
    if !target.exists() {
        return Ok(false);
    }
    verify_model(&target, tier.pin())?;
    Ok(true)
}

pub fn download_model(tier: SemifModelTier, directory: &Path) -> Result<PathBuf, SemifSetupError> {
    let pin = tier.pin();
    fs::create_dir_all(directory).map_err(|error| SemifSetupError::Io {
        context: format!("create {}", directory.display()),
        message: error.to_string(),
    })?;

    let target = model_path(tier, directory);
    if target.exists() {
        match verify_size(&target, pin.gguf_bytes) {
            Ok(()) => {
                verify_model(&target, pin)?;
                return Ok(target);
            }
            Err(_) => {
                let partial = partial_model_path(tier, directory);
                if partial.exists() {
                    fs::remove_file(&target).map_err(|error| SemifSetupError::Io {
                        context: format!("remove incomplete {}", target.display()),
                        message: error.to_string(),
                    })?;
                } else {
                    fs::rename(&target, &partial).map_err(|error| SemifSetupError::Io {
                        context: format!(
                            "move incomplete {} to {}",
                            target.display(),
                            partial.display()
                        ),
                        message: error.to_string(),
                    })?;
                }
            }
        }
    }

    let partial = partial_model_path(tier, directory);
    run_command(
        Command::new("curl")
            .arg("--fail")
            .arg("--location")
            .args([
                "--connect-timeout",
                "30",
                "--max-time",
                "3600",
                "--retry",
                "3",
            ])
            .arg("--continue-at")
            .arg("-")
            .arg("--output")
            .arg(&partial)
            .arg(pin.gguf_url()),
        "download pinned GGUF",
    )?;

    verify_model(&partial, pin)?;
    fs::rename(&partial, &target).map_err(|error| SemifSetupError::Io {
        context: format!("move {} to {}", partial.display(), target.display()),
        message: error.to_string(),
    })?;

    Ok(target)
}

fn partial_model_path(tier: SemifModelTier, directory: &Path) -> PathBuf {
    let pin = tier.pin();
    directory.join(format!("{}.part", pin.gguf_file))
}

fn verify_model(path: &Path, pin: SemifModelPin) -> Result<(), SemifSetupError> {
    verify_size(path, pin.gguf_bytes)?;
    let hash = || -> Result<String, std::io::Error> {
        let mut file = fs::File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        // sha2 0.11 digests no longer implement LowerHex; render each byte.
        Ok(hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    };
    let actual = hash().map_err(|e| SemifSetupError::Io {
        context: format!("hash {}", path.display()),
        message: e.to_string(),
    })?;
    if actual != pin.gguf_sha256 {
        return Err(SemifSetupError::Io { context:format!("verify {}",path.display()),message:format!("SHA-256 mismatch: expected {}, got {actual}; remove the corrupt file and rerun setup",pin.gguf_sha256) });
    }
    Ok(())
}

fn verify_size(path: &Path, expected: u64) -> Result<(), SemifSetupError> {
    let actual = fs::metadata(path)
        .map_err(|error| SemifSetupError::Io {
            context: format!("stat {}", path.display()),
            message: error.to_string(),
        })?
        .len();

    if actual != expected {
        return Err(SemifSetupError::UnexpectedModelSize {
            path: path.to_path_buf(),
            expected,
            actual,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_size_corrupt_model_is_rejected_before_execution() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("model.gguf");
        fs::write(&file, b"bad!").unwrap();
        let mut pin = SemifModelTier::Phone.pin();
        pin.gguf_bytes = 4;
        assert!(verify_model(&file, pin)
            .unwrap_err()
            .to_string()
            .contains("SHA-256 mismatch"));
    }

    #[test]
    fn model_paths_are_deterministic() {
        let root = Path::new(".local/models");
        assert_eq!(
            model_path(SemifModelTier::Phone, root),
            root.join("Qwen3-0.6B-Q8_0.gguf")
        );
        assert_eq!(
            partial_model_path(SemifModelTier::Phone, root),
            root.join("Qwen3-0.6B-Q8_0.gguf.part")
        );
    }
}
