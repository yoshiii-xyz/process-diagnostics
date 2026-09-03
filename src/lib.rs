//! Linux-first process execution diagnostics with bounded, replayable transcripts.
//!
//! The library executes one exact program and argument vector. It never
//! constructs or invokes a shell, task runner, or command pipeline.

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{self, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_CAPTURE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_TRANSCRIPT_BYTES: usize = 24 * 1024 * 1024;
pub const DEFAULT_TIMEOUT_MS: u64 = 60_000;
pub const MAX_TIMEOUT_MS: u64 = 10 * 60 * 1_000;
pub const DEFAULT_KILL_GRACE_MS: u64 = 200;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessGroupPolicy {
    #[default]
    NewProcessGroup,
    CurrentProcessGroup,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancellationPolicy {
    #[default]
    TerminateThenKill,
    KillImmediately,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeoutPolicy {
    pub limit_ms: u64,
    pub kill_grace_ms: u64,
}

impl Default for TimeoutPolicy {
    fn default() -> Self {
        Self {
            limit_ms: DEFAULT_TIMEOUT_MS,
            kill_grace_ms: DEFAULT_KILL_GRACE_MS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessSpec {
    pub program: String,
    pub args: Vec<String>,
    pub inherit_environment: bool,
    pub explicit_environment: BTreeMap<String, String>,
    pub process_group: ProcessGroupPolicy,
    pub cancellation: CancellationPolicy,
    pub timeout: TimeoutPolicy,
}

impl ProcessSpec {
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
            inherit_environment: true,
            explicit_environment: BTreeMap::new(),
            process_group: ProcessGroupPolicy::default(),
            cancellation: CancellationPolicy::default(),
            timeout: TimeoutPolicy::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitSummary {
    pub outcome: String,
    pub code: Option<i32>,
    pub signal: Option<i32>,
    pub timed_out: bool,
    pub cancelled: bool,
    pub spawn_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transcript {
    pub schema_version: u32,
    pub spec: ProcessSpec,
    pub duration_ms: u64,
    #[serde(with = "base64_bytes")]
    pub stdout: Vec<u8>,
    #[serde(with = "base64_bytes")]
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
    pub exit: ExitSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopReason {
    TimedOut,
    Cancelled,
}

#[derive(Debug)]
struct Capture {
    bytes: Vec<u8>,
    truncated: bool,
}

/// Execute one exact argv vector with bounded output capture.
///
/// If `cancel` becomes true while the child is running, the configured
/// cancellation policy is applied. The inherited environment is not copied
/// into the transcript; only explicit environment entries in the spec are
/// recorded.
pub fn execute_process(spec: &ProcessSpec, cancel: Option<&AtomicBool>) -> Transcript {
    let started = Instant::now();
    let mut command = Command::new(&spec.program);
    command.args(&spec.args);
    if !spec.inherit_environment {
        command.env_clear();
    }
    for (key, value) in &spec.explicit_environment {
        command.env(key, value);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    #[cfg(unix)]
    if spec.process_group == ProcessGroupPolicy::NewProcessGroup {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::setpgid(0, 0) == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return Transcript {
                schema_version: SCHEMA_VERSION,
                spec: spec.clone(),
                duration_ms: elapsed_ms(started),
                stdout: Vec::new(),
                stderr: Vec::new(),
                stdout_truncated: false,
                stderr_truncated: false,
                exit: ExitSummary {
                    outcome: "spawn_error".to_owned(),
                    code: None,
                    signal: None,
                    timed_out: false,
                    cancelled: false,
                    spawn_error: Some(error.to_string()),
                },
            };
        }
    };

    #[cfg(unix)]
    if spec.process_group == ProcessGroupPolicy::NewProcessGroup {
        set_parent_process_group(child.id());
    }

    let stdout_reader = child
        .stdout
        .take()
        .map(|stdout| thread::spawn(move || read_capture(stdout)));
    let stderr_reader = child
        .stderr
        .take()
        .map(|stderr| thread::spawn(move || read_capture(stderr)));

    let (status, stop_reason) = wait_for_child(&mut child, spec, cancel);
    let stdout = join_capture(stdout_reader);
    let stderr = join_capture(stderr_reader);
    let exit = summarize_exit(status.as_ref(), stop_reason);

    Transcript {
        schema_version: SCHEMA_VERSION,
        spec: spec.clone(),
        duration_ms: elapsed_ms(started),
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        stdout_truncated: stdout.truncated,
        stderr_truncated: stderr.truncated,
        exit,
    }
}

/// Encode a transcript as bounded JSON.
pub fn transcript_json(transcript: &Transcript) -> Result<String, String> {
    let rendered = serde_json::to_string(transcript).map_err(|error| error.to_string())?;
    if rendered.len() > MAX_TRANSCRIPT_BYTES {
        return Err(format!(
            "transcript is {} bytes, maximum is {} bytes",
            rendered.len(),
            MAX_TRANSCRIPT_BYTES
        ));
    }
    Ok(rendered)
}

/// Decode a previously recorded transcript without executing its command.
pub fn replay_transcript(input: &str) -> Result<Transcript, String> {
    if input.len() > MAX_TRANSCRIPT_BYTES {
        return Err(format!(
            "transcript is {} bytes, maximum is {} bytes",
            input.len(),
            MAX_TRANSCRIPT_BYTES
        ));
    }
    let transcript: Transcript = serde_json::from_str(input).map_err(|error| error.to_string())?;
    if transcript.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "unsupported transcript schema {}",
            transcript.schema_version
        ));
    }
    if transcript.stdout.len() > MAX_CAPTURE_BYTES || transcript.stderr.len() > MAX_CAPTURE_BYTES {
        return Err("transcript capture exceeds the configured bound".to_owned());
    }
    Ok(transcript)
}

/// Render a transcript without rerunning or inspecting the recorded command.
pub fn explain_transcript(transcript: &Transcript) -> String {
    let command = std::iter::once(transcript.spec.program.as_str())
        .chain(transcript.spec.args.iter().map(String::as_str))
        .collect::<Vec<_>>()
        .join(" ");
    let mut lines = vec![
        format!("command: {command}"),
        format!("outcome: {}", transcript.exit.outcome),
        format!("duration_ms: {}", transcript.duration_ms),
        format!("stdout_bytes: {}", transcript.stdout.len()),
        format!("stderr_bytes: {}", transcript.stderr.len()),
        format!("stdout_truncated: {}", transcript.stdout_truncated),
        format!("stderr_truncated: {}", transcript.stderr_truncated),
    ];
    if let Some(code) = transcript.exit.code {
        lines.push(format!("exit_code: {code}"));
    }
    if let Some(signal) = transcript.exit.signal {
        lines.push(format!("signal: {signal}"));
    }
    if let Some(error) = &transcript.exit.spawn_error {
        lines.push(format!("spawn_error: {error}"));
    }
    lines.join("\n")
}

fn wait_for_child(
    child: &mut Child,
    spec: &ProcessSpec,
    cancel: Option<&AtomicBool>,
) -> (Option<ExitStatus>, Option<StopReason>) {
    let deadline = Instant::now()
        .checked_add(Duration::from_millis(spec.timeout.limit_ms))
        .unwrap_or_else(Instant::now);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return (Some(status), None),
            Ok(None) => {}
            Err(_error) => {
                let _ = child.kill();
                let _ = child.wait();
                return (None, Some(StopReason::Cancelled));
            }
        }

        if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
            if let Ok(Some(status)) = child.try_wait() {
                return (Some(status), None);
            }
            let status = stop_child(child, spec, StopReason::Cancelled);
            return (status, Some(StopReason::Cancelled));
        }
        if Instant::now() >= deadline {
            if let Ok(Some(status)) = child.try_wait() {
                return (Some(status), None);
            }
            let status = stop_child(child, spec, StopReason::TimedOut);
            return (status, Some(StopReason::TimedOut));
        }
        thread::sleep(Duration::from_millis(5));
    }
}

fn stop_child(child: &mut Child, spec: &ProcessSpec, _reason: StopReason) -> Option<ExitStatus> {
    #[cfg(unix)]
    if spec.process_group == ProcessGroupPolicy::NewProcessGroup {
        let pid = child.id();
        if spec.cancellation == CancellationPolicy::TerminateThenKill {
            signal_process_group(pid, libc::SIGTERM);
            let grace_deadline = Instant::now()
                .checked_add(Duration::from_millis(spec.timeout.kill_grace_ms))
                .unwrap_or_else(Instant::now);
            loop {
                if let Ok(Some(status)) = child.try_wait() {
                    return Some(status);
                }
                if Instant::now() >= grace_deadline {
                    break;
                }
                thread::sleep(Duration::from_millis(5));
            }
        }
        signal_process_group(pid, libc::SIGKILL);
        return child.wait().ok();
    }

    let _ = child.kill();
    child.wait().ok()
}

fn summarize_exit(status: Option<&ExitStatus>, reason: Option<StopReason>) -> ExitSummary {
    let timed_out = reason == Some(StopReason::TimedOut);
    let cancelled = reason == Some(StopReason::Cancelled);
    let (code, signal) = status.map(exit_code_and_signal).unwrap_or((None, None));
    let outcome = if timed_out {
        "timeout"
    } else if cancelled {
        "cancelled"
    } else if status.is_none() {
        "wait_error"
    } else if status.is_some_and(ExitStatus::success) {
        "success"
    } else if signal.is_some() {
        "signaled"
    } else {
        "nonzero"
    };
    ExitSummary {
        outcome: outcome.to_owned(),
        code,
        signal,
        timed_out,
        cancelled,
        spawn_error: None,
    }
}

fn read_capture(mut reader: impl Read) -> Capture {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut truncated = false;
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                let remaining = MAX_CAPTURE_BYTES.saturating_sub(bytes.len());
                if read <= remaining {
                    bytes.extend_from_slice(&buffer[..read]);
                } else {
                    bytes.extend_from_slice(&buffer[..remaining]);
                    truncated = true;
                }
            }
            Err(_) => break,
        }
    }
    Capture { bytes, truncated }
}

fn join_capture(handle: Option<thread::JoinHandle<Capture>>) -> Capture {
    handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or(Capture {
            bytes: Vec::new(),
            truncated: false,
        })
}

fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u64::MAX as u128) as u64
}

#[cfg(unix)]
fn set_parent_process_group(pid: u32) {
    let _ = unsafe { libc::setpgid(pid as libc::pid_t, pid as libc::pid_t) };
}

#[cfg(unix)]
fn signal_process_group(pid: u32, signal: libc::c_int) {
    let _ = unsafe { libc::kill(-(pid as libc::pid_t), signal) };
}

#[cfg(unix)]
fn exit_code_and_signal(status: &ExitStatus) -> (Option<i32>, Option<i32>) {
    use std::os::unix::process::ExitStatusExt;
    (status.code(), status.signal())
}

#[cfg(not(unix))]
fn exit_code_and_signal(status: &ExitStatus) -> (Option<i32>, Option<i32>) {
    (status.code(), None)
}

mod base64_bytes {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&base64::engine::general_purpose::STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    fn shell(script: &str) -> ProcessSpec {
        ProcessSpec::new("/bin/sh", vec!["-c".to_owned(), script.to_owned()])
    }

    #[test]
    fn success_output_is_captured() {
        let transcript = execute_process(&shell("printf success"), None);
        assert_eq!(transcript.exit.outcome, "success");
        assert_eq!(transcript.stdout, b"success");
        assert!(transcript.stderr.is_empty());
    }

    #[test]
    fn nonzero_exit_is_classified() {
        let transcript = execute_process(&shell("exit 7"), None);
        assert_eq!(transcript.exit.outcome, "nonzero");
        assert_eq!(transcript.exit.code, Some(7));
    }

    #[cfg(unix)]
    #[test]
    fn signal_exit_is_classified() {
        let transcript = execute_process(&shell("kill -TERM $$"), None);
        assert_eq!(transcript.exit.outcome, "signaled");
        assert_eq!(transcript.exit.signal, Some(libc::SIGTERM));
    }

    #[test]
    fn timeout_is_classified_and_child_stops() {
        let mut spec = shell("sleep 1");
        spec.timeout.limit_ms = 30;
        let transcript = execute_process(&spec, None);
        assert_eq!(transcript.exit.outcome, "timeout");
        assert!(transcript.exit.timed_out);
    }

    #[cfg(unix)]
    #[test]
    fn process_group_policy_stops_background_child() {
        let mut spec = shell("sleep 10 & echo $!; wait");
        spec.timeout.limit_ms = 40;
        let transcript = execute_process(&spec, None);
        assert_eq!(transcript.exit.outcome, "timeout");
        let child_pid = String::from_utf8_lossy(&transcript.stdout)
            .trim()
            .parse::<libc::pid_t>()
            .unwrap();
        let still_running = unsafe { libc::kill(child_pid, 0) == 0 };
        assert!(
            !still_running,
            "background child survived process-group stop"
        );
    }

    #[test]
    fn large_output_is_bounded() {
        let spec = ProcessSpec::new(
            "/usr/bin/head",
            vec![
                "-c".to_owned(),
                "10000000".to_owned(),
                "/dev/zero".to_owned(),
            ],
        );
        let transcript = execute_process(&spec, None);
        assert_eq!(transcript.exit.outcome, "success");
        assert_eq!(transcript.stdout.len(), MAX_CAPTURE_BYTES);
        assert!(transcript.stdout_truncated);
    }

    #[test]
    fn binary_and_unicode_output_are_lossless() {
        let binary = execute_process(&shell("printf '\\377\\000\\001'"), None);
        assert_eq!(binary.stdout, [255, 0, 1]);
        let unicode = execute_process(
            &shell("printf '\\303\\251llo \\344\\270\\226\\347\\225\\214'"),
            None,
        );
        assert_eq!(unicode.stdout, b"\xC3\xA9llo \xE4\xB8\x96\xE7\x95\x8C");
    }

    #[test]
    fn explicit_environment_can_replace_inheritance() {
        let mut spec = shell("printf %s \"$PD_EXPLICIT\"");
        spec.inherit_environment = false;
        spec.explicit_environment
            .insert("PD_EXPLICIT".to_owned(), "visible".to_owned());
        let transcript = execute_process(&spec, None);
        assert_eq!(transcript.exit.outcome, "success");
        assert_eq!(transcript.stdout, b"visible");
        assert!(!transcript.spec.inherit_environment);
    }

    #[test]
    fn cancellation_race_is_bounded() {
        let cancel = AtomicBool::new(true);
        let transcript = execute_process(&shell("true"), Some(&cancel));
        assert!(matches!(
            transcript.exit.outcome.as_str(),
            "success" | "cancelled"
        ));
        assert!(transcript.duration_ms < 1_000);
    }

    #[test]
    fn transcript_round_trip_is_replayable() {
        let transcript = execute_process(&shell("printf replay"), None);
        let json = transcript_json(&transcript).unwrap();
        let replayed = replay_transcript(&json).unwrap();
        assert_eq!(replayed, transcript);
        assert!(explain_transcript(&replayed).contains("outcome: success"));
    }

    #[test]
    fn missing_program_is_reported_without_panic() {
        let transcript = execute_process(&ProcessSpec::new("/no/such/program", Vec::new()), None);
        assert_eq!(transcript.exit.outcome, "spawn_error");
        assert!(transcript.exit.spawn_error.is_some());
    }
}
