# Bible Reading Planner

A desktop app that generates customized Bible reading plans as CSV files (with an optional calendar export).

Built in Rust using [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) (egui).

## Features

- **Multiple reading tracks** — read different sets of books at different paces in parallel (e.g. OT once, NT twice); each active track gets its own column in the output
- **Flexible book selection** — quick-select groups (Entire Bible, OT, NT, Pentateuch, History, Poetry, Major/Minor Prophets, Gospels+Acts, Epistles, Revelation) or pick individual books; groups toggle on/off with a single click, and a Clear button resets a track
- **Repetition control** — read a track once, twice, or any number of times
- **Date-range or fixed-duration scheduling** — specify a start/end date or a total number of days
- **Skip days** — exclude any day(s) of the week (e.g. skip Sundays)
- **Balanced daily reading** — chapters are grouped to minimize the length of the single worst day (and, among splits that tie on that, to minimize variance), rather than a naive average-based pass that could dump an oversized chapter (e.g. Psalm 119) onto whatever short chapters came right before it
- **Exact-length plans** — day allocation always sums to exactly the requested duration, and any leftover catch-up days are spread proportionally across the schedule instead of clustering into one long run
- **Catch-up day warning** — if a book selection has far fewer chapters than the plan's duration (25%+ of days would otherwise be catch-up days), you're warned before generating and can choose to proceed anyway or go back and adjust the schedule
- **Reading length output** — optionally include each day's reading length in the CSV, shown as either an estimated word count or estimated minutes (at a reading speed you set)
- **Optional weekday and header columns** — show the day of the week as its own column, and/or add a header row to the CSV, both off by default
- **Calendar export** — optionally also write an `.ics` file (one all-day event per reading day) for import into Google Calendar, Apple Calendar, or a phone's calendar app; an info button explains how
- **Light/dark theme and adjustable UI scale**
- **Custom output folder and filename**, with buttons to open the generated CSV and/or calendar file immediately after generation
- **Persistent settings** — preferences (theme, UI scale, output folder, reading speed, output-format toggles) are saved and restored automatically; the plan itself (book/track selections, date range, duration) always starts fresh so you don't inherit a stale plan from your last session

## Requirements

- Rust (stable, 1.92+)
- `bible.csv` must be present in the working directory (included in this repo)

## Building and running

```sh
cargo run --release
```

The app must be run from the directory containing `bible.csv`. Settings are saved to `bible_planner_config.json` in the same directory.

## Output

Plans are written as CSV files (named `reading_plan_<timestamp>.csv` by default, or a custom name you choose) under the configured output folder (`reading_plan/` by default). Each row is one reading day:

```
Mon Jan 6 2027,Genesis 10-14,2658
Tue Jan 7 2027,Genesis 15-18,2339
```

If "Include weekday as its own column" is on, the weekday appears as its own leading column instead of being folded into the date. If "Include column header row" is on, a header row (`Date`/`Day`, `Weekday`, `Reading`/`Track 1`, `Track 2`, ..., `Words`/`Minutes`) is written first. For multi-track plans, each active track gets its own column. The trailing number, when reading length is enabled, is either an estimated word count or estimated minutes, depending on your setting.

If calendar export is enabled and the plan uses a date range, an `.ics` file with the same name is written alongside the CSV.

## Data source

`bible.csv` contains per-chapter character counts for all 66 books of the Protestant Bible. The planner uses these to distribute reading evenly by length rather than by raw chapter count, converting to an estimated word count by dividing by the average English word length (5 characters) wherever word counts or reading-time estimates are shown.
