use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LogCaptureError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Log directory not found: {0}")]
    DirNotFound(PathBuf),
}

type Result<T> = std::result::Result<T, LogCaptureError>;

/// Severity level of a detected game error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorSeverity {
    Warn,
    Error,
    Fatal,
}

/// Source subsystem that produced an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorSource {
    Wine,
    Dxvk,
    MoltenVK,
    Metal,
    D3DMetal,
    Unknown,
}

/// A structured error detected by scanning game logs.
#[derive(Debug, Clone)]
pub struct GameError {
    pub severity: ErrorSeverity,
    pub source: ErrorSource,
    pub message: String,
    pub line_number: usize,
}

/// Captures and manages Wine/DXVK log output for a game session.
pub struct LogCapture {
    pub log_dir: PathBuf,
    pub game_id: String,
}

impl LogCapture {
    /// Create a new LogCapture that stores logs under `log_dir/game_id/`.
    pub fn new(log_dir: PathBuf, game_id: &str) -> Self {
        tracing::debug!(log_dir = %log_dir.display(), game_id = %game_id, "LogCapture initialized");
        Self {
            log_dir,
            game_id: game_id.to_string(),
        }
    }

    /// Build environment variables that redirect Wine and DXVK log output
    /// to the game-specific log directory.
    pub fn setup_log_env(&self) -> HashMap<String, String> {
        let log_path = self.get_log_path();

        // Ensure the log directory exists.
        if let Err(e) = std::fs::create_dir_all(&log_path) {
            tracing::warn!("Failed to create log directory: {e}");
        }

        let mut env = HashMap::new();

        // Wine debug channel configuration — suppress noise by default,
        // keeping only errors and warnings.
        env.insert("WINEDEBUG".to_string(), "-all,+err,+warn".to_string());

        // DXVK log output path (DXVK appends its own filename).
        env.insert(
            "DXVK_LOG_PATH".to_string(),
            log_path.to_string_lossy().to_string(),
        );

        env
    }

    /// Returns the directory where logs for this game are stored.
    pub fn get_log_path(&self) -> PathBuf {
        self.log_dir.join(&self.game_id)
    }

    /// Read the last `lines` lines from the most recently modified log file
    /// in the game's log directory.
    pub fn read_recent_log(&self, lines: usize) -> Result<Vec<String>> {
        let log_path = self.get_log_path();
        if !log_path.exists() {
            return Err(LogCaptureError::DirNotFound(log_path));
        }

        // Find the most recently modified file in the log directory.
        let latest = std::fs::read_dir(&log_path)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .max_by_key(|e| {
                e.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            });

        let latest = match latest {
            Some(entry) => entry.path(),
            None => return Ok(Vec::new()),
        };

        let file = std::fs::File::open(&latest)?;
        let reader = BufReader::new(file);
        let all_lines: Vec<String> = reader.lines().collect::<std::io::Result<Vec<_>>>()?;

        let start = all_lines.len().saturating_sub(lines);
        Ok(all_lines[start..].to_vec())
    }

    /// Remove all log files for this game.
    pub fn clear_logs(&self) -> Result<()> {
        let log_path = self.get_log_path();
        if log_path.exists() {
            std::fs::remove_dir_all(&log_path)?;
            tracing::info!("Cleared logs for game {}", self.game_id);
        }
        Ok(())
    }

    /// Scan the most recent log file for common error patterns from Wine,
    /// DXVK, MoltenVK, and Metal.
    pub fn detect_errors(&self) -> Result<Vec<GameError>> {
        tracing::debug!(game_id = %self.game_id, "Scanning logs for errors");
        let log_path = self.get_log_path();
        if !log_path.exists() {
            return Err(LogCaptureError::DirNotFound(log_path));
        }

        // Find the most recently modified log file.
        let latest = std::fs::read_dir(&log_path)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|ft| ft.is_file()).unwrap_or(false))
            .max_by_key(|e| {
                e.metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
            });

        let latest = match latest {
            Some(entry) => entry.path(),
            None => return Ok(Vec::new()),
        };

        let file = std::fs::File::open(&latest)?;
        let reader = BufReader::new(file);
        let mut errors = Vec::new();

        for (idx, line_result) in reader.lines().enumerate() {
            let line = match line_result {
                Ok(l) => l,
                Err(_) => continue,
            };
            let line_number = idx + 1;

            // Wine unimplemented function stubs
            if line.contains("fixme:") {
                errors.push(GameError {
                    severity: ErrorSeverity::Warn,
                    source: ErrorSource::Wine,
                    message: line.clone(),
                    line_number,
                });
            }

            // Wine errors
            if line.contains("err:") {
                errors.push(GameError {
                    severity: ErrorSeverity::Error,
                    source: ErrorSource::Wine,
                    message: line.clone(),
                    line_number,
                });
            }

            // DXVK failures
            if line.contains("DXVK: Failed") {
                errors.push(GameError {
                    severity: ErrorSeverity::Error,
                    source: ErrorSource::Dxvk,
                    message: line.clone(),
                    line_number,
                });
            }

            // Metal shader errors
            if line.contains("MTLLibrary") {
                errors.push(GameError {
                    severity: ErrorSeverity::Error,
                    source: ErrorSource::Metal,
                    message: line.clone(),
                    line_number,
                });
            }

            // MoltenVK errors
            if line.contains("MVK_ERROR") || line.contains("MoltenVK error") {
                errors.push(GameError {
                    severity: ErrorSeverity::Error,
                    source: ErrorSource::MoltenVK,
                    message: line.clone(),
                    line_number,
                });
            }

            // D3DMetal errors
            if line.contains("D3DM_ERROR") || line.contains("d3dmetal: error") {
                errors.push(GameError {
                    severity: ErrorSeverity::Error,
                    source: ErrorSource::D3DMetal,
                    message: line.clone(),
                    line_number,
                });
            }

            // Fatal crashes
            if line.contains("FATAL") || line.contains("Segmentation fault") {
                errors.push(GameError {
                    severity: ErrorSeverity::Fatal,
                    source: ErrorSource::Unknown,
                    message: line.clone(),
                    line_number,
                });
            }
        }

        tracing::info!(
            game_id = %self.game_id,
            errors_found = errors.len(),
            fatal = errors.iter().filter(|e| e.severity == ErrorSeverity::Fatal).count(),
            "Error detection complete"
        );
        Ok(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setup_log_env() {
        let tmp = tempfile::tempdir().unwrap();
        let capture = LogCapture::new(tmp.path().to_path_buf(), "game123");
        let env = capture.setup_log_env();

        assert_eq!(env.get("WINEDEBUG"), Some(&"-all,+err,+warn".to_string()));
        assert!(env.contains_key("DXVK_LOG_PATH"));
        // Log directory should have been created
        assert!(capture.get_log_path().exists());
    }

    #[test]
    fn test_get_log_path() {
        let tmp = tempfile::tempdir().unwrap();
        let capture = LogCapture::new(tmp.path().to_path_buf(), "myGame");
        let log_path = capture.get_log_path();
        assert!(log_path.ends_with("myGame"));
    }

    #[test]
    fn test_detect_errors_wine_fixme() {
        let tmp = tempfile::tempdir().unwrap();
        let capture = LogCapture::new(tmp.path().to_path_buf(), "test");
        let log_dir = capture.get_log_path();
        std::fs::create_dir_all(&log_dir).unwrap();
        std::fs::write(log_dir.join("wine.log"), "fixme:d3d11 some unimplemented thing\n").unwrap();

        let errors = capture.detect_errors().unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].severity, ErrorSeverity::Warn);
        assert_eq!(errors[0].source, ErrorSource::Wine);
    }

    #[test]
    fn test_detect_errors_wine_err() {
        let tmp = tempfile::tempdir().unwrap();
        let capture = LogCapture::new(tmp.path().to_path_buf(), "test");
        let log_dir = capture.get_log_path();
        std::fs::create_dir_all(&log_dir).unwrap();
        std::fs::write(log_dir.join("wine.log"), "err:ntdll something broke\n").unwrap();

        let errors = capture.detect_errors().unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].severity, ErrorSeverity::Error);
        assert_eq!(errors[0].source, ErrorSource::Wine);
    }

    #[test]
    fn test_detect_errors_dxvk() {
        let tmp = tempfile::tempdir().unwrap();
        let capture = LogCapture::new(tmp.path().to_path_buf(), "test");
        let log_dir = capture.get_log_path();
        std::fs::create_dir_all(&log_dir).unwrap();
        std::fs::write(log_dir.join("dxvk.log"), "DXVK: Failed to create device\n").unwrap();

        let errors = capture.detect_errors().unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].source, ErrorSource::Dxvk);
    }

    #[test]
    fn test_detect_errors_moltenvk() {
        let tmp = tempfile::tempdir().unwrap();
        let capture = LogCapture::new(tmp.path().to_path_buf(), "test");
        let log_dir = capture.get_log_path();
        std::fs::create_dir_all(&log_dir).unwrap();
        std::fs::write(log_dir.join("mvk.log"), "MVK_ERROR: something failed\n").unwrap();

        let errors = capture.detect_errors().unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].source, ErrorSource::MoltenVK);
    }

    #[test]
    fn test_detect_errors_d3dmetal() {
        let tmp = tempfile::tempdir().unwrap();
        let capture = LogCapture::new(tmp.path().to_path_buf(), "test");
        let log_dir = capture.get_log_path();
        std::fs::create_dir_all(&log_dir).unwrap();
        std::fs::write(log_dir.join("d3dm.log"), "D3DM_ERROR: shader compile fail\n").unwrap();

        let errors = capture.detect_errors().unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].source, ErrorSource::D3DMetal);
    }

    #[test]
    fn test_detect_errors_metal() {
        let tmp = tempfile::tempdir().unwrap();
        let capture = LogCapture::new(tmp.path().to_path_buf(), "test");
        let log_dir = capture.get_log_path();
        std::fs::create_dir_all(&log_dir).unwrap();
        std::fs::write(log_dir.join("metal.log"), "MTLLibrary error: bad shader\n").unwrap();

        let errors = capture.detect_errors().unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].source, ErrorSource::Metal);
    }

    #[test]
    fn test_detect_errors_fatal() {
        let tmp = tempfile::tempdir().unwrap();
        let capture = LogCapture::new(tmp.path().to_path_buf(), "test");
        let log_dir = capture.get_log_path();
        std::fs::create_dir_all(&log_dir).unwrap();
        std::fs::write(log_dir.join("crash.log"), "Segmentation fault at 0x0\n").unwrap();

        let errors = capture.detect_errors().unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].severity, ErrorSeverity::Fatal);
    }

    #[test]
    fn test_detect_errors_no_log_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let capture = LogCapture::new(tmp.path().to_path_buf(), "nonexistent");

        let result = capture.detect_errors();
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_errors_empty_log_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let capture = LogCapture::new(tmp.path().to_path_buf(), "test");
        let log_dir = capture.get_log_path();
        std::fs::create_dir_all(&log_dir).unwrap();

        let errors = capture.detect_errors().unwrap();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_clear_logs() {
        let tmp = tempfile::tempdir().unwrap();
        let capture = LogCapture::new(tmp.path().to_path_buf(), "test");
        let log_dir = capture.get_log_path();
        std::fs::create_dir_all(&log_dir).unwrap();
        std::fs::write(log_dir.join("test.log"), "data").unwrap();

        capture.clear_logs().unwrap();
        assert!(!log_dir.exists());
    }
}
