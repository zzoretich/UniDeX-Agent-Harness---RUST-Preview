# 0004: Embedded Chromium, rendered off-screen

Status: accepted
Date: 2026-11-16

## Context

The app needs an embedded browser, one the agent can drive with the `computer_*` tools and the user can watch. GPUI
draws the app window, so how a page is placed in it decides what can appear over the page.

## Options considered

| Option | Trade-off |
|---|---|
| Chromium rendered off-screen in a helper process, drawn as an element in the app window (chosen) | App UI can draw over the page; web content stays out of the app's process; input, text entry, cursors and popups must be forwarded by hand. |
| Chromium as a native child view | Less forwarding work, but it sits above everything GPUI draws, so ask cards, popovers and the command palette cannot appear over a page. |
| The system web view | Not Chromium on macOS or Linux, and cannot be driven the way the `computer_*` tools need. Rejected. |
| An external browser window | Simplest, but not embedded. Kept only as the fallback for a platform where embedding fails. |

## Decision

Chromium is embedded. It renders off-screen in a helper process and is drawn as an element in the app window.

## Who decided

The maintainer decided to embed Chromium. The method, off-screen rendering in a helper process, is a design call, to be
confirmed by the M0 trial.

## Consequences

App UI such as ask cards, popovers and the command palette can appear over a page, and web content stays out of the app
process. Input, text entry, cursors and popups must be forwarded by hand, which is real work. An external browser window
remains the fallback on a platform where embedding fails.

## Revisit when

After decision 0012 from the M0 browser trial, which confirms or replaces the off-screen method.
