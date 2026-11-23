---
name: "Meeting Recorder — Windows 11 Desktop"
description: "A native-inspired, local-first Windows 11 desktop design system for dual-track audio recording and real-time Thai transcription."
---

## Overview

Meeting Recorder is a local-first Windows 11 desktop productivity tool built in Rust and egui/eframe for simultaneous system audio and microphone capture with live OpenThai ASR transcription. The visual language is restrained, functional, and native-desktop inspired—prioritizing scannability during long meetings, unmistakable recording state visibility, and crisp typographic hierarchy over decorative flourishes.

The default polarity is **Dark Mode**, engineered around deep slate neutrals (`#111318` ground, `#171A21` sidebar, `#1D222B` surface, `#242A35` elevated surface) with high-legibility `#F1F5F9` foreground text and a disciplined `#3B82F6` primary accent. A paired **Light Mode** concept is included for daytime and system-theme parity.

## Colors

Colors are organized into mode-independent semantic/status tokens (`success`, `warning`, `error`, `brand-blue`) and mode-specific `light:` and `dark:` surface/foreground hierarchies. Semantic hues (`success`, `warning`, `error`) are strictly reserved for recording status, audio metering, and engine/device health—never used decoratively.

- **Surfaces (Dark):** Deep neutral slate tiers step outward from `#111318` (main canvas) to `#171A21` (navigation rail), `#1D222B` (cards/panels), and `#242A35` (elevated controls, hover states, and dialogs).
- **Surfaces (Light):** Crisp cool-neutral tiers step from `#F8FAFC` (main canvas) and `#F1F5F9` (navigation rail) to `#FFFFFF` (cards/panels) and `#E2E8F0` (elevated controls).
- **Borders:** Subtle 1px hairline borders (`outline`) separate panels and tables without heavy shadows.

```yaml
colors:
  brand-blue: "#3B82F6"
  success: "#22C55E"
  on-success: "#052E16"
  warning: "#F59E0B"
  on-warning: "#451A03"
  error: "#EF4444"
  on-error: "#1C1C1C"
  dark:
    primary: "#2563EB"
    on-primary: "#FFFFFF"
    primary-container: "#1E3A5F"
    on-primary-container: "#DBEAFE"
    background: "#111318"
    on-background: "#F1F5F9"
    sidebar: "#171A21"
    on-sidebar: "#F1F5F9"
    surface: "#1D222B"
    on-surface: "#F1F5F9"
    surface-elevated: "#242A35"
    on-surface-elevated: "#F1F5F9"
    surface-variant: "#242A35"
    on-surface-variant: "#94A3B8"
    outline: "#2E3543"
    outline-variant: "#1F242E"
    scrim: "#000000"
  light:
    primary: "#2563EB"
    on-primary: "#FFFFFF"
    primary-container: "#DBEAFE"
    on-primary-container: "#1E3A8A"
    background: "#F8FAFC"
    on-background: "#0F172A"
    sidebar: "#F1F5F9"
    on-sidebar: "#0F172A"
    surface: "#FFFFFF"
    on-surface: "#0F172A"
    surface-elevated: "#E2E8F0"
    on-surface-elevated: "#0F172A"
    surface-variant: "#F1F5F9"
    on-surface-variant: "#475569"
    outline: "#CBD5E1"
    outline-variant: "#E2E8F0"
    scrim: "#0F172A"
```

## Typography

Interface copy uses **Inter** for clean UI legibility, paired with **Noto Sans Thai** fallback for live Thai transcript segments, and **JetBrains Mono** for elapsed timers, timestamps, file paths, and sample-rate metrics. All numeric readouts, timers, and table columns use `tabular-nums` so live updates never jitter.

Icons: Lucide

```yaml
typography:
  display-timer:
    fontFamily: "JetBrains Mono"
    fontSize: 36px
    fontWeight: 600
    lineHeight: 44px
    letterSpacing: -0.5px
  display-timer-mobile:
    fontFamily: "JetBrains Mono"
    fontSize: 28px
    fontWeight: 600
    lineHeight: 36px
    letterSpacing: -0.25px
  headline-lg:
    fontFamily: "Inter"
    fontSize: 22px
    fontWeight: 600
    lineHeight: 28px
    letterSpacing: -0.2px
  headline-md:
    fontFamily: "Inter"
    fontSize: 16px
    fontWeight: 600
    lineHeight: 22px
    letterSpacing: -0.1px
  title-sm:
    fontFamily: "Inter"
    fontSize: 14px
    fontWeight: 600
    lineHeight: 20px
    letterSpacing: 0px
  body-md:
    fontFamily: "Inter"
    fontSize: 14px
    fontWeight: 400
    lineHeight: 22px
    letterSpacing: 0px
  body-thai:
    fontFamily: "Noto Sans Thai"
    fontSize: 15px
    fontWeight: 400
    lineHeight: 26px
    letterSpacing: 0px
  body-sm:
    fontFamily: "Inter"
    fontSize: 13px
    fontWeight: 400
    lineHeight: 18px
    letterSpacing: 0px
  label-lg:
    fontFamily: "Inter"
    fontSize: 13px
    fontWeight: 500
    lineHeight: 18px
    letterSpacing: 0px
  label-sm:
    fontFamily: "Inter"
    fontSize: 11px
    fontWeight: 500
    lineHeight: 16px
    letterSpacing: 0.2px
  mono-sm:
    fontFamily: "JetBrains Mono"
    fontSize: 12px
    fontWeight: 400
    lineHeight: 18px
    letterSpacing: 0px
```

## Layout & Spacing

Designed for Windows 11 desktop proportions with a minimum target of **1200 × 760 px** and fluid resizing. A persistent left navigation rail (`220px` expanded / `64px` collapsed) anchors the shell across all screens. The primary Recorder screen uses a `35% / 65%` split-view inside the main content area, while Settings uses a secondary `220px` category rail beside a single-column form area.

Spacing follows a strict **4px / 8px** desktop grid: `4px` micro-gaps, `8px` inline control spacing, `12px`–`16px` panel padding, and `20px`–`24px` section margins.

```yaml
spacing:
  gutter-xs: 4px
  gutter-sm: 8px
  gutter-md: 12px
  gutter-lg: 16px
  gutter-xl: 24px
  section: 32px
  page-margin: 24px
```

## Elevation & Depth

Hierarchy is established through **tonal surface stepping** and crisp **1px low-contrast outlines** rather than heavy drop shadows or frosted glass blurs—ensuring straightforward 1:1 implementation in Rust `egui / eframe`. Modal confirmation dialogs use a subtle dark scrim and a single restrained elevation border.

## Shapes

Corner radii are moderate and functional, echoing native Windows 11 desktop controls: `4px` for meter segments and inline tags, `6px` for buttons, inputs, and dropdowns, and `8px` for panels and cards.

```yaml
rounded:
  sm: 4px
  DEFAULT: 6px
  md: 8px
  lg: 10px
  full: 9999px
```

## Motion & Micro-interaction

Transitions are crisp, immediate, and strictly restrained—no decorative loops, bouncing cards, or pulsing dots. Interactive hover/focus states settle in `100ms`, and panel/dialog reveals settle in `150ms`. Audio level meters update cleanly without CSS layout reflow.

```yaml
motion:
  state-change:
    duration: 100ms
    delay: 0ms
    timingFunction: ease-out
  surface-enter:
    duration: 150ms
    delay: 0ms
    timingFunction: cubic-bezier(0.2, 0, 0, 1)
```

## Voice & Content Strategy

- **Tone:** Technical, calm, and direct—clear native Windows utility copy without marketing buzzwords.
- **Capitalisation:** Sentence case for all labels, table headers, settings descriptions, and buttons (`Start recording`, `Open transcript`, `Confirm before deleting recordings`).
- **Terminology:** Consistent domain terms: `System Audio`, `Microphone`, `OpenThai ASR`, `Partial`, `Finalized`, `Recordings`, `Transcripts`.
- **Numbers & Dates:** ISO-style or clear desktop formats (`2026-11-23 09:30`, `00:42:18`, `48.2 MB`, `48,000 Hz`).

## Components

Component implementations live as individual HTML sticker sheet files in
`components/`. Each file contains the canonical, styled HTML components with
`<!-- COMPONENT: name -->` comment markers. Read each file before use.

- **Badge** → `@components/badge.html`: `badge-numeric`, `badge-dot`
- **Button** → `@components/button.html`: `button-primary`, `button-icon`, `button-with-icon`
- **Input Field** → `@components/input-field.html`: `input-field-text`, `input-field-icon`
- **Select** → `@components/select.html`: `select-native`, `select-custom`
- **Table** → `@components/table.html`: `table-standard`, `table-interactive`, `table-selectable`
- **Toggle** → `@components/toggle.html`: `toggle-standard`, `toggle-label`, `toggle-description`
