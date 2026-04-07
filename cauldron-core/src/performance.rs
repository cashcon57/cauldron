use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::time::Instant;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PerfError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("No snapshots recorded")]
    NoData,
}

type Result<T> = std::result::Result<T, PerfError>;

/// Configuration for performance monitoring during a game session.
#[derive(Debug, Clone)]
pub struct PerfConfig {
    /// Enable the Metal performance HUD overlay.
    pub metal_hud: bool,
    /// Enable frame timing collection via DXVK/MoltenVK.
    pub frame_timing: bool,
    /// How often (in seconds) to log a performance summary.
    pub log_interval_secs: u32,
    /// Whether to capture FPS data into snapshots.
    pub capture_fps: bool,
}

impl Default for PerfConfig {
    fn default() -> Self {
        Self {
            metal_hud: false,
            frame_timing: false,
            log_interval_secs: 30,
            capture_fps: false,
        }
    }
}

/// A single performance sample captured at a point in time.
#[derive(Debug, Clone)]
pub struct PerfSnapshot {
    /// ISO-8601 timestamp of this sample.
    pub timestamp: String,
    /// Current instantaneous FPS.
    pub fps_current: f32,
    /// Running average FPS since monitoring started.
    pub fps_avg: f32,
    /// Minimum FPS observed so far.
    pub fps_min: f32,
    /// Maximum FPS observed so far.
    pub fps_max: f32,
    /// Frame time in milliseconds for the most recent frame.
    pub frame_time_ms: f32,
    /// GPU utilization percentage, if available from the driver.
    pub gpu_utilization: Option<f32>,
    /// Resident memory usage in megabytes.
    pub memory_used_mb: u64,
}

/// Aggregated performance statistics across all recorded snapshots.
#[derive(Debug, Clone)]
pub struct PerfSummary {
    /// Total number of frames recorded.
    pub total_frames: u64,
    /// Total duration of the monitoring session in seconds.
    pub duration_secs: f64,
    /// Average FPS across all snapshots.
    pub avg_fps: f32,
    /// Minimum FPS observed.
    pub min_fps: f32,
    /// Maximum FPS observed.
    pub max_fps: f32,
    /// 1st percentile FPS (worst-case).
    pub p1_fps: f32,
    /// 5th percentile FPS.
    pub p5_fps: f32,
    /// Average frame time in milliseconds.
    pub avg_frame_time_ms: f32,
    /// Maximum (worst) frame time in milliseconds.
    pub max_frame_time_ms: f32,
}

/// Collects and aggregates performance data during a game session.
pub struct PerfMonitor {
    pub config: PerfConfig,
    pub snapshots: Vec<PerfSnapshot>,
    pub start_time: Instant,
    pub frame_count: u64,
}

impl PerfMonitor {
    /// Create a new performance monitor with the given configuration.
    pub fn new(config: PerfConfig) -> Self {
        Self {
            config,
            snapshots: Vec::new(),
            start_time: Instant::now(),
            frame_count: 0,
        }
    }

    /// Build environment variables that enable performance overlays and
    /// telemetry in Wine, DXVK, and Metal.
    pub fn build_perf_env(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();

        if self.config.metal_hud {
            env.insert("MTL_HUD_ENABLED".to_string(), "1".to_string());
        }

        if self.config.frame_timing {
            env.insert(
                "DXVK_HUD".to_string(),
                "fps,frametimes,gpuload".to_string(),
            );
            env.insert("DXVK_LOG_LEVEL".to_string(), "info".to_string());
            env.insert("MVK_CONFIG_LOG_LEVEL".to_string(), "1".to_string());
        }

        env
    }

    /// Record a performance snapshot.
    pub fn record_snapshot(&mut self, snapshot: PerfSnapshot) {
        self.frame_count += 1;
        tracing::debug!(
            frame = self.frame_count,
            fps = snapshot.fps_current,
            frame_time_ms = snapshot.frame_time_ms,
            "Performance snapshot recorded"
        );
        self.snapshots.push(snapshot);
    }

    /// Compute an aggregated summary of all recorded snapshots.
    ///
    /// Returns a `PerfSummary` with percentile and average statistics.
    /// If no snapshots have been recorded the summary will contain zeroes.
    pub fn get_summary(&self) -> PerfSummary {
        if self.snapshots.is_empty() {
            return PerfSummary {
                total_frames: 0,
                duration_secs: self.start_time.elapsed().as_secs_f64(),
                avg_fps: 0.0,
                min_fps: 0.0,
                max_fps: 0.0,
                p1_fps: 0.0,
                p5_fps: 0.0,
                avg_frame_time_ms: 0.0,
                max_frame_time_ms: 0.0,
            };
        }

        let duration_secs = self.start_time.elapsed().as_secs_f64();
        let count = self.snapshots.len() as f32;

        let avg_fps: f32 = self.snapshots.iter().map(|s| s.fps_current).sum::<f32>() / count;
        let min_fps = self
            .snapshots
            .iter()
            .map(|s| s.fps_current)
            .fold(f32::INFINITY, f32::min);
        let max_fps = self
            .snapshots
            .iter()
            .map(|s| s.fps_current)
            .fold(f32::NEG_INFINITY, f32::max);

        let avg_frame_time_ms: f32 =
            self.snapshots.iter().map(|s| s.frame_time_ms).sum::<f32>() / count;
        let max_frame_time_ms = self
            .snapshots
            .iter()
            .map(|s| s.frame_time_ms)
            .fold(f32::NEG_INFINITY, f32::max);

        // Percentile computation: sort FPS values ascending, pick indices.
        let mut sorted_fps: Vec<f32> = self.snapshots.iter().map(|s| s.fps_current).collect();
        sorted_fps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let n = sorted_fps.len();
        let p1_idx = ((n as f64) * 0.01).floor() as usize;
        let p5_idx = ((n as f64) * 0.05).floor() as usize;

        let p1_fps = sorted_fps[p1_idx.min(n - 1)];
        let p5_fps = sorted_fps[p5_idx.min(n - 1)];

        let summary = PerfSummary {
            total_frames: self.frame_count,
            duration_secs,
            avg_fps,
            min_fps,
            max_fps,
            p1_fps,
            p5_fps,
            avg_frame_time_ms,
            max_frame_time_ms,
        };
        tracing::info!(
            total_frames = summary.total_frames,
            avg_fps = format!("{:.1}", summary.avg_fps),
            min_fps = format!("{:.1}", summary.min_fps),
            max_fps = format!("{:.1}", summary.max_fps),
            duration_secs = format!("{:.1}", summary.duration_secs),
            "Performance summary computed"
        );
        summary
    }

    /// Export all recorded snapshots to a CSV file.
    pub fn export_to_csv(&self, path: &Path) -> Result<()> {
        if self.snapshots.is_empty() {
            return Err(PerfError::NoData);
        }

        let mut file = std::fs::File::create(path)?;

        // Header
        writeln!(
            file,
            "timestamp,fps_current,fps_avg,fps_min,fps_max,frame_time_ms,gpu_utilization,memory_used_mb"
        )?;

        for snap in &self.snapshots {
            let gpu = match snap.gpu_utilization {
                Some(v) => format!("{v:.1}"),
                None => String::new(),
            };
            writeln!(
                file,
                "{},{:.1},{:.1},{:.1},{:.1},{:.2},{},{}",
                snap.timestamp,
                snap.fps_current,
                snap.fps_avg,
                snap.fps_min,
                snap.fps_max,
                snap.frame_time_ms,
                gpu,
                snap.memory_used_mb,
            )?;
        }

        tracing::info!("Exported {} snapshots to {}", self.snapshots.len(), path.display());
        Ok(())
    }

    /// Clear all recorded monitoring data and reset the frame counter.
    pub fn clear(&mut self) {
        self.snapshots.clear();
        self.frame_count = 0;
        self.start_time = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot(fps: f32, frame_time: f32) -> PerfSnapshot {
        PerfSnapshot {
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            fps_current: fps,
            fps_avg: fps,
            fps_min: fps,
            fps_max: fps,
            frame_time_ms: frame_time,
            gpu_utilization: None,
            memory_used_mb: 1024,
        }
    }

    #[test]
    fn test_perf_monitor_new() {
        let monitor = PerfMonitor::new(PerfConfig::default());
        assert!(monitor.snapshots.is_empty());
        assert_eq!(monitor.frame_count, 0);
    }

    #[test]
    fn test_record_snapshot() {
        let mut monitor = PerfMonitor::new(PerfConfig::default());
        monitor.record_snapshot(make_snapshot(60.0, 16.6));
        monitor.record_snapshot(make_snapshot(30.0, 33.3));

        assert_eq!(monitor.snapshots.len(), 2);
        assert_eq!(monitor.frame_count, 2);
    }

    #[test]
    fn test_summary_empty() {
        let monitor = PerfMonitor::new(PerfConfig::default());
        let summary = monitor.get_summary();
        assert_eq!(summary.total_frames, 0);
        assert_eq!(summary.avg_fps, 0.0);
    }

    #[test]
    fn test_summary_with_data() {
        let mut monitor = PerfMonitor::new(PerfConfig::default());
        monitor.record_snapshot(make_snapshot(60.0, 16.6));
        monitor.record_snapshot(make_snapshot(30.0, 33.3));
        monitor.record_snapshot(make_snapshot(90.0, 11.1));

        let summary = monitor.get_summary();
        assert_eq!(summary.total_frames, 3);
        assert!((summary.avg_fps - 60.0).abs() < 0.1);
        assert_eq!(summary.min_fps, 30.0);
        assert_eq!(summary.max_fps, 90.0);
        assert!((summary.avg_frame_time_ms - 20.333).abs() < 0.1);
        assert_eq!(summary.max_frame_time_ms, 33.3);
    }

    #[test]
    fn test_build_perf_env_empty() {
        let monitor = PerfMonitor::new(PerfConfig::default());
        let env = monitor.build_perf_env();
        assert!(env.is_empty());
    }

    #[test]
    fn test_build_perf_env_metal_hud() {
        let config = PerfConfig {
            metal_hud: true,
            frame_timing: false,
            ..PerfConfig::default()
        };
        let monitor = PerfMonitor::new(config);
        let env = monitor.build_perf_env();
        assert_eq!(env.get("MTL_HUD_ENABLED"), Some(&"1".to_string()));
    }

    #[test]
    fn test_build_perf_env_frame_timing() {
        let config = PerfConfig {
            metal_hud: false,
            frame_timing: true,
            ..PerfConfig::default()
        };
        let monitor = PerfMonitor::new(config);
        let env = monitor.build_perf_env();
        assert!(env.contains_key("DXVK_HUD"));
        assert!(env.contains_key("DXVK_LOG_LEVEL"));
        assert!(env.contains_key("MVK_CONFIG_LOG_LEVEL"));
    }

    #[test]
    fn test_clear() {
        let mut monitor = PerfMonitor::new(PerfConfig::default());
        monitor.record_snapshot(make_snapshot(60.0, 16.6));
        assert_eq!(monitor.frame_count, 1);

        monitor.clear();
        assert_eq!(monitor.frame_count, 0);
        assert!(monitor.snapshots.is_empty());
    }

    #[test]
    fn test_export_to_csv() {
        let tmp = tempfile::tempdir().unwrap();
        let csv_path = tmp.path().join("perf.csv");
        let mut monitor = PerfMonitor::new(PerfConfig::default());
        monitor.record_snapshot(make_snapshot(60.0, 16.6));

        monitor.export_to_csv(&csv_path).unwrap();
        let content = std::fs::read_to_string(&csv_path).unwrap();
        assert!(content.contains("timestamp,fps_current"));
        assert!(content.contains("60.0"));
    }

    #[test]
    fn test_export_to_csv_no_data() {
        let tmp = tempfile::tempdir().unwrap();
        let csv_path = tmp.path().join("perf.csv");
        let monitor = PerfMonitor::new(PerfConfig::default());

        let result = monitor.export_to_csv(&csv_path);
        assert!(result.is_err());
    }
}
