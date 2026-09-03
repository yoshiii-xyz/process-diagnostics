# Design

## Process specification

`ProcessSpec` names one program and an ordered argument vector. It also records
whether the child inherits the caller environment, explicit key-value
overrides, process group policy, cancellation policy, and timeout policy. The
library passes those fields to `std::process::Command` and never creates a
shell string.

## Capture and transcript

stdout and stderr are piped and drained by separate reader threads while the
parent polls the child. Each stream retains at most 8 MiB and continues
draining after the bound so a verbose child does not block on a full pipe.
Output is base64 encoded in `Transcript`, which keeps binary output lossless.
The transcript records explicit environment entries but does not snapshot
inherited environment values.

## Completion and control

The parent checks for normal completion before checking cancellation or the
timeout deadline. This ordering makes a completed child win a close
completion-control race. A timeout or cancellation first sends the configured
signal to a new process group on Unix, waits the grace period, then sends
SIGKILL if needed. Current-process-group mode falls back to the child handle's
kill behavior.

## Exit classification

The result distinguishes `success`, `nonzero`, `signaled`, `timeout`,
`cancelled`, `spawn_error`, and `wait_error`. Unix signal numbers come from
`ExitStatusExt`. The classification reports what this invocation observed; it
does not claim complete knowledge of descendants, kernel scheduling, or
external side effects.

## Replay

`replay_transcript` validates the schema and capture bounds, then
`explain_transcript` renders the saved facts without opening the recorded
program or its working paths. A transcript is evidence of one invocation, not
a command to execute.
