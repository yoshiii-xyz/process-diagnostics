use clap::{Parser, Subcommand, ValueEnum};
use process_diagnostics::{
    CancellationPolicy, DEFAULT_KILL_GRACE_MS, DEFAULT_TIMEOUT_MS, MAX_TIMEOUT_MS,
    ProcessGroupPolicy, ProcessSpec, TimeoutPolicy, execute_process, explain_transcript,
    replay_transcript, transcript_json,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
    Text,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum GroupMode {
    New,
    Current,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CancelMode {
    TerminateThenKill,
    KillImmediately,
}

#[derive(Debug, Parser)]
#[command(
    name = "process-diagnostics",
    version,
    about = "Capture bounded diagnostics for one exact process invocation"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Execute one exact program and argument vector.
    Run {
        #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
        format: OutputFormat,
        #[arg(long, default_value_t = DEFAULT_TIMEOUT_MS)]
        timeout_ms: u64,
        #[arg(long, default_value_t = DEFAULT_KILL_GRACE_MS)]
        kill_grace_ms: u64,
        #[arg(long)]
        cancel_after_ms: Option<u64>,
        #[arg(long, value_enum, default_value_t = GroupMode::New)]
        process_group: GroupMode,
        #[arg(long, value_enum, default_value_t = CancelMode::TerminateThenKill)]
        cancellation: CancelMode,
        #[arg(long = "env", value_name = "KEY=VALUE")]
        environment: Vec<String>,
        #[arg(long)]
        clear_env: bool,
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
    /// Explain a saved transcript without rerunning its command.
    Replay {
        transcript: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
}

fn main() -> ExitCode {
    match execute(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("process-diagnostics: {error}");
            ExitCode::from(2)
        }
    }
}

fn execute(cli: Cli) -> Result<u8, Box<dyn std::error::Error>> {
    match cli.command {
        Commands::Run {
            format,
            timeout_ms,
            kill_grace_ms,
            cancel_after_ms,
            process_group,
            cancellation,
            environment,
            clear_env,
            command,
        } => {
            if command.is_empty() {
                return Err("a command is required after --".into());
            }
            let timeout_ms = timeout_ms.min(MAX_TIMEOUT_MS);
            let mut spec = ProcessSpec::new(command[0].clone(), command[1..].to_vec());
            spec.inherit_environment = !clear_env;
            spec.explicit_environment = parse_environment(&environment)?;
            spec.process_group = match process_group {
                GroupMode::New => ProcessGroupPolicy::NewProcessGroup,
                GroupMode::Current => ProcessGroupPolicy::CurrentProcessGroup,
            };
            spec.cancellation = match cancellation {
                CancelMode::TerminateThenKill => CancellationPolicy::TerminateThenKill,
                CancelMode::KillImmediately => CancellationPolicy::KillImmediately,
            };
            spec.timeout = TimeoutPolicy {
                limit_ms: timeout_ms,
                kill_grace_ms,
            };
            let cancel = Arc::new(AtomicBool::new(false));
            if let Some(delay_ms) = cancel_after_ms {
                let cancel_for_thread = Arc::clone(&cancel);
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(delay_ms));
                    cancel_for_thread.store(true, Ordering::Relaxed);
                });
            }
            let transcript = execute_process(&spec, Some(&cancel));
            render(&transcript, format)?;
            Ok(match transcript.exit.outcome.as_str() {
                "success" => 0,
                "spawn_error" | "wait_error" => 2,
                _ => 1,
            })
        }
        Commands::Replay { transcript, format } => {
            let input = fs::read_to_string(transcript)?;
            let transcript = replay_transcript(&input)?;
            render(&transcript, format)?;
            Ok(0)
        }
    }
}

fn parse_environment(values: &[String]) -> Result<BTreeMap<String, String>, String> {
    let mut environment = BTreeMap::new();
    for value in values {
        let Some((key, val)) = value.split_once('=') else {
            return Err(format!("environment entry {value:?} must use KEY=VALUE"));
        };
        if key.is_empty() {
            return Err("environment key cannot be empty".to_owned());
        }
        environment.insert(key.to_owned(), val.to_owned());
    }
    Ok(environment)
}

fn render(
    transcript: &process_diagnostics::Transcript,
    format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        OutputFormat::Json => println!("{}", transcript_json(transcript)?),
        OutputFormat::Text => println!("{}", explain_transcript(transcript)),
    }
    Ok(())
}
