# 0005: Platform support in tiers

Status: accepted
Date: 2026-11-16

## Context

The app must run on macOS, Linux and Windows. Only a macOS machine is available for hands-on checks, so a claim of
support could outrun its evidence.

## Options considered

The design lists no alternatives to the tiering. It states support in tiers so that a claim never outruns its evidence:

| Option | Trade-off |
|---|---|
| Tier 1 for macOS (Apple silicon), Tier 2 for Linux (x64) and Windows (x64) (chosen) | Every claim matches its evidence; Linux and Windows are labelled "CI-verified, not yet run on real hardware" until a tester confirms them. |

## Decision

Tier 1 is macOS on Apple silicon, with a hands-on check by the maintainer at every gate. Tier 2 is Linux (x64) and
Windows (x64), verified by CI only: the build, the unit tests, the view-model tests, and end-to-end tests that drive the
real app through its automation socket. Intel Macs, ARM Linux and ARM Windows come later.

## Who decided

The maintainer required all three platforms. The tiering is a design call, approved with the design.

## Consequences

Linux and Windows builds carry the "CI-verified, not yet run on real hardware" label until a tester confirms them. M0
checks whether virtual machines on the development machine can render the app; if they can, they become a hands-on step
for Tier 2. Known exceptions at design time: screen reader support in GPUI is unconfirmed, screenshot-based UI tests run
on macOS first, and without signing identities supplied as CI secrets a platform's builds are unsigned and say so.

## Revisit when

A tester confirms a Tier 2 platform on real hardware, or the M0 virtual machine check shows Tier 2 can be checked
hands-on, or Intel Macs, ARM Linux or ARM Windows are brought into scope.
