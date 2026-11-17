# Security policy

UniDeX Rust Preview is pre-release. There is nothing to install yet, no supported version and no security guarantee.
Reports are still welcome: a design flaw found now is the cheapest one to fix.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting on this repository: open the **Security** tab and choose **Report a
vulnerability**. That creates a private advisory that only the maintainers can see. Please do not put details in a
public issue, pull request or discussion. There is no security email address.

Include what you found, how to reproduce it, the commit or build, the platform, and the impact you expect. A proof of
concept that uses `example.com` hosts and invented data is ideal; never send real secrets or personal data.

Reports are handled on a best-effort basis; expect an acknowledgement within about a week. Fixes land on the default
branch. Once releases exist, each fix ships with a published advisory that credits the reporter unless they ask
otherwise. There is no bug bounty.

## Scope

In scope:
- Code in this repository, its build and CI configuration, and anything it publishes (nightly builds, releases, the
  update feed) once those exist.
- A way around any control in the [threat model](docs/security/THREAT-MODEL.md), including controls planned for
  milestones not yet built. A flaw in the design is a valid report.
- Prompt injection that leads to an effect a control should have stopped: a command run, a file read or written, data
  sent out, a memory saved, money spent.
- Secrets or personal information committed to this repository.

Out of scope:
- A model following injected text when no control was bypassed. If you think a control is missing, report that.
- Attacks that need code already running as the user's own OS account, physical access, or a compromised OS.
- Vulnerabilities in third-party software (add-on agents such as Claude Code or Codex, MCP servers, model servers,
  Chromium, crates). Please report those upstream, and tell us if UniDeX needs to update or work around them.
- The Electron edition of UniDeX, which is a separate project with its own security policy.
- Denial of service through the user's own configuration, and social engineering of the maintainers.

## Threat model

[docs/security/THREAT-MODEL.md](docs/security/THREAT-MODEL.md) lists the assets, the attackers, each security boundary
with its planned controls, the milestone that delivers each control and the test that will prove it. It is updated at
every milestone gate.
