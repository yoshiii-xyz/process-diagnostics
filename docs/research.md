# Research notes

Research date: 2026-09-03.

Primary sources:

- Rust's [`Command`](https://doc.rust-lang.org/std/process/struct.Command.html)
  documents direct program and argument construction and environment methods.
- Rust's [`Child`](https://doc.rust-lang.org/std/process/struct.Child.html)
  documents spawned-child handles, waiting, polling, and termination methods.
- Rust's [`Stdio`](https://doc.rust-lang.org/std/process/struct.Stdio.html)
  documents piped standard streams used by the capture readers.
- Rust's [`ExitStatus`](https://doc.rust-lang.org/std/process/struct.ExitStatus.html)
  documents portable success and exit-code inspection.
- Unix [`ExitStatusExt`](https://doc.rust-lang.org/std/os/unix/process/trait.ExitStatusExt.html)
  documents signal access for Unix exit statuses.
- Unix [`CommandExt::pre_exec`](https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html)
  documents the pre-exec hook used to establish a new process group on the
  Linux release target.
- The JSON [`RFC 8259`](https://www.rfc-editor.org/rfc/rfc8259) defines the
  interchange format that the transcript schema uses.

Distribution signal: Cargo package metadata and a standalone CLI repository
are prepared for crates.io. Package availability or download counts are
distribution signals, not evidence of willingness to pay.

Evidence grade: the cited Rust documentation supports the process APIs and
the documented status model. The transcript schema, output bounds, polling
interval, and process-group policy are design choices verified by this
implementation's Linux tests. They are not universal operating-system
guarantees.

Rejected alternatives include a shell wrapper or task runner, which would
hide command-boundary behavior, and a process supervisor, which would require
broader lifecycle guarantees. Decision: keep the release to one exact local
invocation, bounded streams, explicit control policies, and replayable facts.
