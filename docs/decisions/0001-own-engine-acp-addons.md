# 0001: Own engine, CLIs as optional add-ons over ACP

Status: accepted
Date: 2026-11-16

## Context

The Electron app runs every tool-using agent turn as a `claude` or `codex` command-line process, and about half of its
runtime adapts to those two tools. The new app must work with any model, including local ones, and must not depend on a
vendor's command-line tool or login to function. Frontier-model subscriptions are assumed usable only through the
vendor's own tools.

## Options considered

| Option | Trade-off |
|---|---|
| Own engine, CLIs as optional add-ons (chosen) | Independent by default; subscriptions and cross-review between Claude Code and Codex still work; costs one ACP client. |
| Own engine only | Cleanest; frontier models by API key only; loses cross-review on subscriptions. |
| Keep driving the CLIs, add an own engine beside them | Not independent. Rejected. |

## Decision

The app has its own native engine. Claude Code and Codex are reached as optional add-on engines through the Agent Client
Protocol (ACP).

## Who decided

The maintainer.

## Consequences

The app works with API keys or local models and no vendor tool installed. Subscriptions and cross-review stay available
through the add-ons. The project must build and maintain one ACP client, pin a major version of the
`agent-client-protocol` crate, run a conformance test for each pin, and check and write down the vendor terms for each
add-on in M0.

## Revisit when

The ACP conformance test cannot be kept passing against the pinned major version, or the M0 check of vendor terms finds
that an add-on is not permitted.
