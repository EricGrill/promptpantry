# Security Policy

## Supported Versions

Prompt Pantry is pre-1.0. Security fixes are applied to the latest commit on
`main`.

## Reporting a Vulnerability

Please report suspected vulnerabilities privately through GitHub's vulnerability
reporting flow when it is available for this repository. If that is unavailable,
open a minimal public issue that does not include exploit details or sensitive
data, and ask for a private contact path.

Do not include access tokens, private prompts, credentials, or other secrets in
public issues, pull requests, logs, screenshots, or reproduction archives.

## Scope

Prompt Pantry is a local CLI/TUI that reads local prompt cards, a local
`library.yaml` catalog, local files, and GitHub file URLs supplied by the user.
Reports involving path traversal, unintended file writes/deletes, command
execution, clipboard exposure, or unsafe GitHub source handling are in scope.
