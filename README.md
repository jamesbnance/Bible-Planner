# Bible Reading Planner

A desktop app that generates customized Bible reading plans as CSV files.

Built in Rust using [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) (egui).

## Features

- **Multiple reading tracks** — read different sets of books at different paces in parallel (e.g. OT once, NT twice)
- **Flexible book selection** — quick-select groups (Entire Bible, OT, NT, Pentateuch, Gospels, etc.) or pick individual books; groups toggle on/off with a single click
- **Repetition control** — read a track once, twice, or any number of times
- **Date-range or fixed-duration scheduling** — specify a start/end date or a total number of days
- **Skip days** — exclude any day(s) of the week (e.g. skip Sundays)
- **Word-count output** — optionally include estimated daily word counts in the CSV
- **Open output** — button to open the generated CSV in your default application immediately after generation
- **Persistent settings** — your configuration is saved automatically and restored on next launch

## Requirements

- Rust (stable, 1.92+)
- `bible.csv` must be present in the working directory (included in this repo)

## Building and running

```sh
cargo run --release
```

The app must be run from the directory containing `bible.csv`. Settings are saved to `bible_planner_config.json` in the same directory.

## Output

Plans are written as CSV files named `reading_plan_<timestamp>.csv` in the working directory. Each row is one reading day:

```
Mon, Jan 6, 2027,"Genesis 10-14", 13290
Tue, Jan 7, 2027,"Genesis 15-18", 11697
```

For multi-track plans, each track gets its own column on the same row.

## Data source

`bible.csv` contains per-chapter word counts for all 66 books of the Protestant Bible. The planner uses these counts to distribute reading evenly by length rather than by raw chapter count.
