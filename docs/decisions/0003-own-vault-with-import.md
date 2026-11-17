# 0003: Own vault with a read-only import

Status: accepted
Date: 2026-11-16

## Context

The Electron app keeps its memory and state in a vault. The new app must be independent in data as well as in models,
and preview software must not put a user's real data at risk.

## Options considered

| Option | Trade-off |
|---|---|
| Own vault with a repeatable, read-only import (chosen) | Independent in data as well as models; preview software never writes to the real vault. |
| Share the Electron app's live vault | One memory from day one, but duties fire twice, state sits in the other app's formats, and the tie is permanent. |

## Decision

The new app has its own vault. It can import from the Electron app's vault, repeatedly, and only reads from it.

## Who decided

The maintainer.

## Consequences

The two apps do not share live state, so a change made in one does not appear in the other until the next import. The
import must be repeatable and must never write to the source vault. Data formats that carry over from the Electron app
must be read correctly.

## Revisit when

The preview is no longer a preview and the Electron app is retired, or the repeatable import proves unable to keep the
two vaults in step well enough to use.
