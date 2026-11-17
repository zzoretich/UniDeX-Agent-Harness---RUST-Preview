# 0002: Native Rust UI on GPUI

Status: accepted
Date: 2026-11-16

## Context

The Electron app's UI is about 54,000 lines of hand-written DOM code, with blocking calls in render paths and several
polling loops. None of it translates. The new app must be faster and smoother, and run on macOS, Linux and Windows.

## Options considered

| Option | Trade-off |
|---|---|
| Native Rust UI on GPUI (chosen) | Rust throughout, GPU-drawn, largest smoothness gain; every screen is rebuilt; framework is pre-1.0. |
| Tauri shell with the existing web UI adapted | Fastest to parity and lowest risk; screens stay TypeScript in a web view. |
| egui, Iced, Slint | Weaker fit for a text-heavy app. Rejected. |
| Rust compiled to WebAssembly in a web view | Costs of both columns. Rejected. |
| Rust core swapped under the Electron app | Not a new application. Rejected. |

## Decision

The app shell and every screen are native Rust on GPUI.

## Who decided

The maintainer.

## Consequences

Every screen is rebuilt. GPUI is pre-1.0, so it is consumed as a snapshot of a specific upstream commit; M0 records the
exact source, the pinned versions and the upgrade procedure, and an upgrade is its own change, never mixed with feature
work. The core has no UI code in it, so the shell choice stays reversible.

## Revisit when

The M0 trial shows GPUI failing outright on a platform, or the pinned snapshot cannot be upgraded without rewriting
screens.
