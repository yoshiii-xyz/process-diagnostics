# process-diagnostics

process-diagnostics records what one local process invocation did: exact
program and arguments, environment policy, bounded stdout and stderr, process
group policy, timeout or cancellation policy, duration, and exit
classification.

Status: 0.1.0 implementation pending release evidence.

CI: https://github.com/joshiii-xyz/process-diagnostics/actions

## Install

```text
cargo install process-diagnostics
```

## Quick start

```text
process-diagnostics run -- ./command --flag value
process-diagnostics run --format text --timeout-ms 5000 -- ./command
process-diagnostics replay transcript.json
```

Save the JSON stdout from `run` as a transcript file, then use `replay` to
inspect it without executing the recorded command.

## What it solves

Process failures are often reduced to a line of output and an exit code. The
transcript keeps the command boundary, output streams, binary data, timeout or
cancellation state, signal where observable, and environment policy together.

## How it works

The library passes the program and argument vector directly to
`std::process::Command`. It never constructs a shell command. stdout and
stderr are drained concurrently and retained up to 8 MiB per stream. On Linux,
the default policy puts the child in a new process group so timeout and
cancellation can signal the group.

The transcript encodes output as base64 in a versioned JSON schema. Inherited
environment values are not copied into the transcript. Explicit values are
recorded because they are part of the supplied process specification.

## Commands and library API

The CLI provides `run` and `replay`. The library exposes `ProcessSpec`,
`execute_process`, `Transcript`, `transcript_json`, `replay_transcript`, and
`explain_transcript`. Use `process-diagnostics --help` for the complete option
list.

## Output and exit codes

- Exit code 0 means the command completed successfully, or a transcript was
  replayed.
- Exit code 1 means the command was nonzero, signaled, cancelled, or timed
  out.
- Exit code 2 means the command could not be spawned, a transcript could not
  be decoded, or CLI rendering failed.

Capture is capped at 8 MiB per stream. Transcript JSON is capped at 24 MiB.
Timeouts are capped at 10 minutes by the CLI. Truncation and all configured
policies are recorded.

## Safety and data handling

The tool does not interpret shell metacharacters, expand pipelines, run a task
file, or upload output. A caller can explicitly choose a shell as the program,
but its shell semantics then belong to that supplied program. Transcripts can
contain explicit environment values, paths, command arguments, and error
output, so store them with suitable permissions.

## Limits and non-goals

See [`docs/limits.md`](docs/limits.md). Linux is the only release-tested
platform. Windows job objects, PTYs, cross-platform signal behavior, and
perfect process-tree accounting are extension points, not current claims.

This is not a shell, task runner, pipeline engine, job scheduler, or general
process supervisor.

## Testing and development

See [`CONTRIBUTING.md`](CONTRIBUTING.md) and [`docs/release.md`](docs/release.md)
for the verified command set.

## Research

See [`docs/research.md`](docs/research.md) for the Rust process API source
trail and the distinction between documented behavior and design inference.

## Release and support status

The 0.1.0 release is pending local and hosted evidence. The release record
will be updated only after the exact package, checksum, docs.rs, CI, security,
CodeQL, tag package, and fresh-install checks pass.
