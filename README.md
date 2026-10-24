# Bible Reading Planner

A desktop app — and a web app, running right in the browser — that generates customized Bible reading plans as CSV files (with an optional calendar export).

Built in Rust using [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) (egui). Try it live at **https://jamesbnance.github.io/Bible-Planner/** — no install needed.

## Features

- **Multiple reading tracks** — read different sets of books at different paces in parallel (e.g. OT once, NT twice); each active track gets its own column in the output
- **Flexible book selection** — quick-select groups (Entire Bible, OT, NT, Pentateuch, History, Poetry, Major/Minor Prophets, Gospels+Acts, Epistles, Revelation) or pick individual books; groups toggle on/off with a single click, and a Clear button resets a track
- **Repetition control** — read a track once, twice, or any number of times
- **Per-track pacing** — balance each track's reading length across the plan's duration (the default), or switch a track to a fixed number of chapters every day instead. A fixed-pace track simply stops when it reaches the end of the selected books, leaving the remaining days blank. If it doesn't finish in time, it's cut off at the plan's end date with a warning
- **Date-range or fixed-duration scheduling** — specify a start/end date or a total number of days
- **Skip days** — exclude any day(s) of the week (e.g. skip Sundays)
- **Balanced daily reading** — for tracks using length-balanced pacing, chapters are grouped to minimize the length of the single worst day (and, among splits that tie on that, to minimize variance), rather than a naive average-based pass that could dump an oversized chapter (e.g. Psalm 119) onto whatever short chapters came right before it
- **Exact-length plans** — a length-balanced track's day allocation always sums to exactly the requested duration, and any leftover catch-up days are spread proportionally across the schedule instead of clustering into one long run
- **Catch-up day warning** — if a length-balanced track has far fewer chapters than the plan's duration (10%+ of days would otherwise be catch-up days), you're warned before generating and can choose to proceed anyway or go back and adjust the schedule
- **Configurable catch-up days** — for length-balanced tracks, "Configure Catch-up Days..." offers one of two options: a fixed number of extra catch-up days spread evenly across the schedule (always landing between books, never splitting one's reading), or catch-up days inserted right after specific books you pick, with a count for each, placed after every pass of that book in every track
- **Reading length output** — optionally include each day's reading length in the CSV, shown as either an estimated word count or estimated minutes (at a reading speed you set)
- **Optional weekday and header columns** — show the day of the week as its own column, and/or add a header row to the CSV, both off by default
- **Calendar export** — optionally also write an `.ics` file (one all-day event per reading day) for import into Google Calendar, Apple Calendar, or a phone's calendar app; an info button explains how
- **Light/dark theme and adjustable UI scale**
- **Custom output folder and filename** (native only — see [Running in the browser](#running-in-the-browser)), with buttons to open the generated CSV and/or calendar file immediately after generation
- **Persistent settings** — preferences (theme, UI scale, output folder, reading speed, output-format toggles) are saved and restored automatically — to `bible_planner_config.json` natively, to the browser's local storage on the web; the plan itself (book/track selections, date range, duration) always starts fresh so you don't inherit a stale plan from your last session

## Requirements

- Rust (stable, 1.92+)
- `bible.csv` must be present in the repo root at *build* time — it's compiled directly into the binary (`include_str!`), not read from disk at runtime, so it works the same way natively and on the web

## Building and running

```sh
cargo run --release
```

Settings are saved to `bible_planner_config.json` next to wherever you run the app from; it's rewritten on every generate and gitignored, not meant to be committed.

## Running in the browser

The web build uses [Trunk](https://trunkrs.dev) to compile to WebAssembly:

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
trunk serve    # dev server with live reload, at http://127.0.0.1:8080
trunk build --release    # production build, written to dist/
```

A live copy is also hosted at **https://jamesbnance.github.io/Bible-Planner/**, redeployed automatically (via `.github/workflows/deploy-pages.yml`) on every push to `develop`.

The web build behaves the same as native with a few necessary differences, since there's no filesystem in a browser:
- Generating a plan downloads the CSV (and `.ics`, if enabled) directly through the browser instead of writing to a chosen folder — there's no output-folder picker or "Open Output File"/"Open Calendar File" buttons.
- Settings persist to the browser's local storage instead of `bible_planner_config.json`.

## Output

Plans are produced as CSV files (named `reading_plan_<timestamp>.csv` by default, or a custom name you choose) — written under the configured output folder (`reading_plan/` by default) natively, or downloaded through the browser on the web. Each row is one reading day:

```
Mon Jan 6 2027,Genesis 10-14,2658
Tue Jan 7 2027,Genesis 15-18,2339
```

If "Include weekday as its own column" is on, the weekday appears as its own leading column instead of being folded into the date. If "Include column header row" is on, a header row (`Date`/`Day`, `Weekday`, `Reading`/`Track 1`, `Track 2`, ..., `Words`/`Minutes`) is written first. For multi-track plans, each active track gets its own column. The trailing number, when reading length is enabled, is either an estimated word count or estimated minutes, depending on your setting.

If calendar export is enabled and the plan uses a date range, an `.ics` file is produced alongside the CSV the same way (written to disk natively, downloaded on the web).

## Data source

`bible.csv` contains per-chapter character counts for all 66 books of the Protestant Bible. The planner uses these to distribute reading evenly by length rather than by raw chapter count, converting to an estimated word count by dividing by the average English word length (5 characters) wherever word counts or reading-time estimates are shown.
