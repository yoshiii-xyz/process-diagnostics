# Security policy

## Supported versions

The latest `0.1.x` release receives security fixes while the project remains
in the 0.1 series.

## Reporting a vulnerability

Do not put credentials, private command lines, or private reproducer data in a
public issue. Use GitHub's private vulnerability reporting for this repository
when available. If it is unavailable, open an issue with only a short,
non-sensitive description and request a private contact path.

process-diagnostics can execute any program the operator supplies. It does not
grant privileges or sandbox the child. Transcripts can contain explicit
environment values and command output, so treat them as sensitive when the
invocation is sensitive.
