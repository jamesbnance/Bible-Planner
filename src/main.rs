use chrono::{NaiveDate, Datelike, Utc};
use serde::Deserialize;
use std::fs::File;
use std::error::Error;
use std::collections::HashMap;
use csv::ReaderBuilder;
use eframe::egui;

#[derive(Debug, Deserialize, Clone)]
struct ChapterData {
    pub title: String,
    pub chapters: i32,
    pub length: i32
}

#[derive(Debug, Deserialize, Clone)]
struct IndexData {
    pub index: i32,
    pub title: String,
    pub chapter: i32,
    pub length: i32
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
struct ChaptersDays {
    pub titles: Vec<String>,
    pub chapters: i32,
    pub days: i32
}

#[derive(Debug, Deserialize, Clone)]
struct DailyLength {
    pub length: i32,
    pub day: i32
}

// ── Book data ────────────────────────────────────────────────────────────────

const BOOK_NAMES: [&str; 66] = [
    "Genesis", "Exodus", "Leviticus", "Numbers", "Deuteronomy",
    "Joshua", "Judges", "Ruth", "1 Samuel", "2 Samuel",
    "1 Kings", "2 Kings", "1 Chronicles", "2 Chronicles", "Ezra",
    "Nehemiah", "Esther", "Job", "Psalms", "Proverbs",
    "Ecclesiastes", "Song of Solomon", "Isaiah", "Jeremiah", "Lamentations",
    "Ezekiel", "Daniel", "Hosea", "Joel", "Amos",
    "Obadiah", "Jonah", "Micah", "Nahum", "Habakkuk",
    "Zephaniah", "Haggai", "Zechariah", "Malachi",
    "Matthew", "Mark", "Luke", "John", "Acts",
    "Romans", "1 Corinthians", "2 Corinthians", "Galatians", "Ephesians",
    "Philippians", "Colossians", "1 Thessalonians", "2 Thessalonians", "1 Timothy",
    "2 Timothy", "Titus", "Philemon", "Hebrews", "James",
    "1 Peter", "2 Peter", "1 John", "2 John", "3 John",
    "Jude", "Revelation",
];

// Quick-select groups: (button label, first 0-based index, last 0-based index)
const BOOK_GROUPS: &[(&str, usize, usize)] = &[
    ("Entire Bible",   0,  65),
    ("OT",             0,  38),
    ("NT",            39,  65),
    ("Pentateuch",     0,   4),
    ("History",        5,  16),
    ("Poetry",        17,  21),
    ("Maj. Prophets", 22,  26),
    ("Min. Prophets", 27,  38),
    ("Gospels+Acts",  39,  43),
    ("Epistles",      44,  64),
    ("Revelation",    65,  65),
];

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct ReadingTrack {
    selected: Vec<bool>,
    read_times: u32,
    show_books: bool,
}

impl ReadingTrack {
    fn empty() -> Self {
        Self { selected: vec![false; 66], read_times: 1, show_books: false }
    }

    fn select_range(&mut self, start: usize, end: usize) {
        for i in start..=end { self.selected[i] = true; }
    }

    fn deselect_range(&mut self, start: usize, end: usize) {
        for i in start..=end { self.selected[i] = false; }
    }

    // Draw this track's UI. Returns true if the user clicked Remove.
    fn show(&mut self, ui: &mut egui::Ui, idx: usize, removable: bool) -> bool {
        let mut remove = false;

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("Track {}", idx + 1)).strong());
            if removable && ui.small_button("✕ Remove").clicked() {
                remove = true;
            }
        });

        // Quick-select buttons — highlighted when every book in the group is selected
        ui.horizontal_wrapped(|ui| {
            for &(label, start, end) in BOOK_GROUPS {
                let active = (start..=end).all(|i| self.selected[i]);
                let fill = if active {
                    ui.visuals().selection.bg_fill
                } else {
                    ui.visuals().widgets.inactive.weak_bg_fill
                };
                let text = egui::RichText::new(label)
                    .color(if active { egui::Color32::WHITE } else { ui.visuals().text_color() });
                if ui.add(egui::Button::new(text).fill(fill).small()).clicked() {
                    if active { self.deselect_range(start, end); } else { self.select_range(start, end); }
                }
            }
            if ui.small_button("Clear").clicked() {
                self.selected = vec![false; 66];
            }
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.read_times, 1, "Once");
            ui.radio_value(&mut self.read_times, 2, "Twice");
            let multi = self.read_times >= 3;
            if ui.radio(multi, "Multiple times:").clicked() && !multi {
                self.read_times = 3;
            }
            if multi {
                ui.add(egui::DragValue::new(&mut self.read_times).range(3u32..=52));
            }
        });
        ui.add_space(8.0);

        // Toggle for the individual book list
        let toggle_label = if self.show_books { "Hide individual books" } else { "Show individual books" };
        if ui.small_button(toggle_label).clicked() {
            self.show_books = !self.show_books;
        }

        if self.show_books {
            ui.add_space(6.0);
            // Book checklist — columns adapt to available width; outer ScrollArea handles overflow
            let col_width = 170.0_f32;
            let num_cols = ((ui.available_width() / col_width) as usize).max(1).min(6);
            egui::Grid::new(format!("book_grid_{}", idx))
                .num_columns(num_cols)
                .min_col_width(col_width)
                .show(ui, |ui| {
                    for (i, &name) in BOOK_NAMES.iter().enumerate() {
                        ui.checkbox(&mut self.selected[i], name);
                        if (i + 1) % num_cols == 0 { ui.end_row(); }
                    }
                    if BOOK_NAMES.len() % num_cols != 0 { ui.end_row(); }
                });
        }

        remove
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (y, m) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    NaiveDate::from_ymd_opt(y, m, 1)
        .and_then(|d| d.pred_opt())
        .map_or(31, |d| d.day())
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Bible Reading Planner",
        options,
        Box::new(|_cc| {
            let app: BiblePlannerApp = std::fs::read_to_string("bible_planner_config.json")
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();
            Ok(Box::new(app))
        }),
    )
}

// ── GUI state ────────────────────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
struct BiblePlannerApp {
    tracks: Vec<ReadingTrack>,
    use_date_range: bool,
    start_year: i32,
    start_month: u32,
    start_day: u32,
    end_year: i32,
    end_month: u32,
    end_day: u32,
    skip_days: [bool; 7],
    duration: i32,
    include_length: bool,

    // Session-only state — not persisted
    #[serde(skip)]
    last_output: Option<String>,
    #[serde(skip)]
    status: String,
    #[serde(skip)]
    status_is_error: bool,
}

impl Default for BiblePlannerApp {
    fn default() -> Self {
        let mut track = ReadingTrack::empty();
        track.select_range(0, 65);

        let mut skip_days = [false; 7];
        skip_days[0] = true; // Sunday

        Self {
            tracks: vec![track],
            use_date_range: true,
            start_year: 2027,
            start_month: 1,
            start_day: 1,
            end_year: 2027,
            end_month: 12,
            end_day: 31,
            skip_days,
            duration: 365,
            include_length: true,
            last_output: None,
            status: String::new(),
            status_is_error: false,
        }
    }
}

// ── Drawing the UI ───────────────────────────────────────────────────────────
//
// eframe calls `update` roughly 60× per second.  Every call rebuilds the
// entire UI from scratch using the current values in `self`.  There are no
// callbacks or event listeners — widgets return whether they were interacted
// with right where they are drawn.

impl eframe::App for BiblePlannerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(20, 16))
            .show(ui, |ui| { self.panel_contents(ui); });
    }
}

impl BiblePlannerApp {
    fn panel_contents(&mut self, ui: &mut egui::Ui) {
        // More vertical breathing room between widgets
        ui.spacing_mut().item_spacing.y = 6.0;

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Bible Reading Planner");

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // ── Books ──────────────────────────────────────────────────────
            ui.label(egui::RichText::new("Books to Read").strong());
            ui.add_space(4.0);

            let mut to_remove: Option<usize> = None;
            let removable = self.tracks.len() > 1;

            for (idx, track) in self.tracks.iter_mut().enumerate() {
                egui::Frame::new()
                    .inner_margin(egui::Margin::same(12))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(80)))
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        if track.show(ui, idx, removable) {
                            to_remove = Some(idx);
                        }
                    });
                ui.add_space(8.0);
            }
            if let Some(idx) = to_remove {
                self.tracks.remove(idx);
            }
            if ui.button("+ Add Track").clicked() {
                self.tracks.push(ReadingTrack::empty());
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // ── Schedule ───────────────────────────────────────────────────
            ui.label(egui::RichText::new("Schedule").strong());
            ui.add_space(2.0);

            // radio_value(&mut field, value_when_selected, "label")
            // The field is set to `value_when_selected` when this radio is clicked.
            ui.radio_value(&mut self.use_date_range, true,  "Date range");
            ui.radio_value(&mut self.use_date_range, false, "Fixed duration (days)");
            ui.add_space(4.0);

            if self.use_date_range {
                const MONTHS: [&str; 12] = ["Jan","Feb","Mar","Apr","May","Jun",
                                            "Jul","Aug","Sep","Oct","Nov","Dec"];

                // Clamp stored days to the real maximum for the selected month/year.
                let start_max = days_in_month(self.start_year, self.start_month);
                self.start_day = self.start_day.min(start_max);
                let end_max = days_in_month(self.end_year, self.end_month);
                self.end_day = self.end_day.min(end_max);

                // `Grid` lines up labels and controls in neat columns.
                egui::Grid::new("dates_grid").num_columns(4).show(ui, |ui| {
                    ui.label("Start date:");
                    ui.add(egui::DragValue::new(&mut self.start_year).range(2020..=2100));
                    egui::ComboBox::from_id_salt("start_month")
                        .selected_text(MONTHS[(self.start_month - 1) as usize])
                        .show_ui(ui, |ui| {
                            for (i, &name) in MONTHS.iter().enumerate() {
                                ui.selectable_value(&mut self.start_month, (i + 1) as u32, name);
                            }
                        });
                    ui.add(egui::DragValue::new(&mut self.start_day).range(1..=start_max).prefix("Day "));
                    ui.end_row();

                    ui.label("End date:");
                    ui.add(egui::DragValue::new(&mut self.end_year).range(2020..=2100));
                    egui::ComboBox::from_id_salt("end_month")
                        .selected_text(MONTHS[(self.end_month - 1) as usize])
                        .show_ui(ui, |ui| {
                            for (i, &name) in MONTHS.iter().enumerate() {
                                ui.selectable_value(&mut self.end_month, (i + 1) as u32, name);
                            }
                        });
                    ui.add(egui::DragValue::new(&mut self.end_day).range(1..=end_max).prefix("Day "));
                    ui.end_row();
                });

                ui.add_space(4.0);
                // `horizontal` places widgets side-by-side on one line.
                ui.horizontal(|ui| {
                    ui.label("Skip weekdays:");
                    let names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
                    for (i, name) in names.iter().enumerate() {
                        ui.checkbox(&mut self.skip_days[i], *name);
                    }
                });
            } else {
                ui.horizontal(|ui| {
                    ui.label("Duration:");
                    ui.add(egui::DragValue::new(&mut self.duration).range(1..=3650).suffix(" days"));
                });
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // ── Options ────────────────────────────────────────────────────
            ui.label(egui::RichText::new("Options").strong());
            ui.add_space(2.0);
            ui.checkbox(&mut self.include_length, "Include daily word count in output");

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // ── Generate button ────────────────────────────────────────────
            // `button` returns a Response; `.clicked()` is true for exactly
            // the one frame the user releases the mouse button.
            if ui.button("  Generate Plan  ").clicked() {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.generate_plan();
                }));
                if result.is_err() {
                    self.status = "Plan generation failed — the settings caused an internal error. Try a different date range or book selection.".to_string();
                    self.status_is_error = true;
                }
            }

            // Status line — green on success, red on error
            if !self.status.is_empty() {
                ui.add_space(8.0);
                let color = if self.status_is_error {
                    egui::Color32::RED
                } else {
                    egui::Color32::from_rgb(0, 160, 0)
                };
                ui.colored_label(color, &self.status);
            }
            if let Some(path) = &self.last_output.clone() {
                if ui.button("Open output file").clicked() {
                    open_file(path);
                }
            }
        });
    }
}

// ── Planning logic called from the GUI ───────────────────────────────────────

impl BiblePlannerApp {
    fn generate_plan(&mut self) {
        // Build book index lists from the track selections
        let mut book_indexes: Vec<Vec<i32>> = Vec::new();
        for track in &self.tracks {
            let indexes: Vec<i32> = (0..66)
                .filter(|&i| track.selected[i])
                .map(|i| (i + 1) as i32)
                .collect();
            if indexes.is_empty() { continue; }
            let repeated: Vec<i32> = indexes.iter()
                .cloned()
                .cycle()
                .take(indexes.len() * track.read_times as usize)
                .collect();
            book_indexes.push(repeated);
        }
        if book_indexes.is_empty() {
            self.status = "Please select at least one book.".to_string();
            self.status_is_error = true;
            return;
        }

        // Build the skip list from the seven checkboxes
        let names: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        let weekdays_to_skip: Vec<&str> = names
            .iter()
            .enumerate()
            .filter(|&(i, _)| self.skip_days[i])
            .map(|(_, &n)| n)
            .collect();

        // Resolve duration and (optionally) a date for each day number
        let (duration, day_dates) = if self.use_date_range {
            let start = match NaiveDate::from_ymd_opt(self.start_year, self.start_month, self.start_day) {
                Some(d) => d,
                None => { self.status = "Invalid start date.".to_string(); self.status_is_error = true; return; }
            };
            let end = match NaiveDate::from_ymd_opt(self.end_year, self.end_month, self.end_day) {
                Some(d) => d,
                None => { self.status = "Invalid end date.".to_string(); self.status_is_error = true; return; }
            };
            if end <= start {
                self.status = "End date must be after start date.".to_string();
                self.status_is_error = true;
                return;
            }
            (get_duration(start, end, &weekdays_to_skip),
             get_day_dates(start, end, &weekdays_to_skip))
        } else {
            (self.duration, Vec::new())
        };

        // Run the existing planning pipeline
        let filename = format!("reading_plan_{}", Utc::now().timestamp());
        let mut combined_plan: Vec<Vec<ChaptersDays>> = Vec::new();
        let mut combined_lengths: Vec<DailyLength> = Vec::new();

        for book_index in book_indexes {
            let bible_data = match get_bible_chapter_data("bible.csv", book_index.clone(), true) {
                Ok(d) => d,
                Err(e) => { self.status = format!("Error reading bible.csv: {}", e); self.status_is_error = true; return; }
            };
            let chapter_data = match get_bible_chapter_data("bible.csv", book_index.clone(), false) {
                Ok(d) => d,
                Err(e) => { self.status = format!("Error reading bible.csv: {}", e); self.status_is_error = true; return; }
            };

            let tcd  = get_books_in_days(bible_data.clone(), duration);
            let tcd2 = get_chapters_days_by_length(chapter_data.clone(), tcd, duration);
            let plan = adjust_days(tcd2, bible_data, duration);

            for (i, day) in plan.iter().enumerate() {
                if combined_plan.len() <= i { combined_plan.push(Vec::new()); }
                combined_plan[i].push(day.clone());
            }

            for daily in get_daily_reading_lengths(plan, chapter_data) {
                if let Some(e) = combined_lengths.iter_mut().find(|e| e.day == daily.day) {
                    e.length += daily.length;
                } else {
                    combined_lengths.push(daily);
                }
            }
        }

        combined_lengths.sort_by_key(|k| k.day);

        let csv_path = format!("{}.csv", filename);
        match write_to_file(&filename, combined_plan, combined_lengths, self.include_length, day_dates) {
            Ok(_) => {
                self.status = format!("Written to {}", csv_path);
                self.last_output = Some(csv_path);
                self.status_is_error = false;
                // Persist settings after a successful generation
                if let Ok(json) = serde_json::to_string_pretty(self) {
                    let _ = std::fs::write("bible_planner_config.json", json);
                }
            }
            Err(e) => {
                self.status = format!("Error: {}", e);
                self.status_is_error = true;
            }
        }
    }
}

fn open_file(path: &str) {
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/c", "start", "", path]).spawn();
}

fn get_duration(start: NaiveDate, end: NaiveDate, weekdays_to_skip: &[&str]) -> i32 {
  let mut current_date = start;
  let mut count = 0;

  while current_date <= end {
      let day_of_week = current_date.weekday().to_string();
      if !weekdays_to_skip.iter().any(|&day| day.eq_ignore_ascii_case(&day_of_week)) {
          count += 1;
      }
      current_date = current_date.succ_opt().expect("Failed to get the next date");
  }

  count
}

// Create a vector with title, number of chapters, total length
fn get_bible_chapter_data(file_path: &str, book_index: Vec<i32>, accumulate: bool) -> Result<Vec<ChapterData>, Box<dyn Error>> {
  let mut data: Vec<ChapterData> = Vec::new();

  for index in book_index {
      // Re-open the CSV and reinitialize the reader to start from the beginning
      let file = File::open(file_path).map_err(|e| {
        eprintln!("Failed to open file '{}': {}", file_path, e);
        e
    })?;
      let mut rdr = ReaderBuilder::new().has_headers(true).from_reader(file);

      // Optionally use a HashMap to accumulate data when aggregation is required
      let mut book_map: HashMap<String, ChapterData> = HashMap::new();

      // Iterate over the CSV records
      for result in rdr.deserialize() {
          let record: IndexData = result?;

          if record.index == index {
              if accumulate {
                  // Aggregated data (equivalent to get_bible_data)
                  let entry = book_map.entry(record.title.clone()).or_insert(ChapterData {
                      title: record.title.clone(),
                      chapters: 0,
                      length: 0,
                  });

                  // Accumulate the chapter and length data
                  entry.chapters += 1;
                  entry.length += record.length;
              } else {
                  // Detailed data (equivalent to get_chapter_data)
                  data.push(ChapterData {
                      title: record.title.clone(),
                      chapters: record.chapter,
                      length: record.length,
                  });
              }
          }
      }

      // If aggregating, add the accumulated data for each book to the data vector
      if accumulate {
          for chapter_data in book_map.into_values() {
              data.push(chapter_data);
          }
      }
  }

  Ok(data)
}

// Determine a vector of the books to read and the number of days allocated for each,
// based on the book indexes and the dates provided. Each element in the returned vector
// represents a group of books to be read within a single day
fn get_books_in_days(bible_data: Vec<ChapterData>, duration: i32) -> Vec<ChaptersDays> {
  let mut result = Vec::new();

  // Temporary storage for book titles that will be combined into a single day's reading.
  let mut temp_titles: Vec<String> = Vec::new();
  // Accumulators for the total number of chapters from and the total number of days required for the temporary book(s).
  let mut temp_chapters: i32 = 0;
  let mut temp_days: f32 = 0.0;

  let total_chapter_count: i32 = bible_data.iter().map(|b| b.chapters).sum();
  // If duration exceeds chapter count, cap planning to chapter count; the extra days become catch-up days in adjust_days.
  let effective_duration = duration.min(total_chapter_count);

  let total_word_count: i32 = bible_data.iter().map(|b| b.length).sum();

  for book in bible_data {
      // Number of days needed to read the current book.
      let days: f32 = (book.length as f32 / total_word_count as f32) * effective_duration as f32;
      // Combine books for partial days.
      if days >= 0.66 {
          // If there are already books scheduled for the current day, finalize the day's schedule and start a new one.
          if !temp_titles.is_empty() {
              push_new_element(&mut result, temp_titles, temp_chapters, temp_days, effective_duration);
              temp_titles = Vec::new();
              temp_chapters = 0;
              temp_days = 0.0;
          }
          push_new_element(&mut result, vec![book.title], book.chapters, days, effective_duration);
      } else {
          // If the book fits within the current day, add it to the temporary storage.
          temp_titles.push(book.title);
          temp_chapters += book.chapters;
          temp_days += days as f32;
          // If the accumulated days for the current day exceed one, finalize the day's schedule and start a new one.
          if temp_days >= 1.0 {
              push_new_element(&mut result, temp_titles, temp_chapters, temp_days, effective_duration);
              temp_titles = Vec::new();
              temp_chapters = 0;
              temp_days = 0.0;
          }
      }
  }
  // After iterating through all books, check if any remaining books must be scheduled for the last day.
  if !temp_titles.is_empty() {
      push_new_element(&mut result, temp_titles, temp_chapters, temp_days, effective_duration);
  }
  result
}

// Used in function get_books_in_days
fn push_new_element(result: &mut Vec<ChaptersDays>, titles: Vec<String>, chapters: i32, days: f32, duration: i32) {
  // Round down for a large number of days, otherwise round to the nearest whole.
  let rdays_threshold = duration as f32 / 30.0;
  let rounded_days = if days > rdays_threshold {
      days.floor() as i32
  } else {
      days.round() as i32
  };

  // Ensure that rounded_days is at least 1
  let rounded_days = rounded_days.max(1);

  let new_element = ChaptersDays { titles, chapters, days: rounded_days };
  result.push(new_element);
}

fn get_chapters_days_by_length(chapter_data: Vec<ChapterData>, titles_chapters_days: Vec<ChaptersDays>, duration: i32) -> Vec<ChaptersDays> {
  let mut title_chapters_days: Vec<ChaptersDays> = Vec::new();
  let mut _days_remaining = duration;
  let mut current_day = 1;

  // Iterate through each set of books and chapters grouped by days
  for books in titles_chapters_days {
    if books.chapters < books.days {
      // More days than chapters: assign one chapter per day; surplus days become catch-up days in adjust_days.
      for chapter in 1..=books.chapters {
          title_chapters_days.push(ChaptersDays {
              titles: books.titles.clone(),
              chapters: chapter,
              days: current_day,
          });
          current_day += 1;
      }
      continue;
    }

    // If exactly one day is assigned, directly assign the book to the current day
    if books.days == 1 {
      title_chapters_days.push(ChaptersDays {
          titles: books.titles.clone(),
          chapters: books.chapters,
          days: current_day,
      });
      _days_remaining -= 1;
      current_day += 1;
      continue;
    }
    assert!(books.titles.len() == 1, "ERROR! Multiple books assigned to a single day!");

    // Load the data for the particular book into chapters
    let title = &books.titles[0];
    let book_days: f64 = books.days as f64;
    let mut chapters: Vec<ChapterData> = Vec::new();

    for data in chapter_data.clone() {
        if &data.title == title {
            chapters.push(data.clone());
        }
        // Stop loading data once the book's last chapter is reached.
        if &data.title == title && data.chapters == books.chapters {
            break;
        }
    }

    let total_words: f64 = chapters.clone().into_iter().map(|chapter| chapter.length as f64).sum();
    let average_words_per_day: f64 = total_words / book_days;

    // Perform binary search to find the optimal distribution of chapters across days.
    let mut low = 0.0;
    let mut high = 1.0;
    let mut tuner = 0.0;
    loop {
        // Group chapters based on the average words per day.
        let mut datasets: Vec<Vec<i32>> = Vec::new();
        let mut current_group_total_words: f64 = 0.0;
        let mut chapter_numbers: Vec<i32> = Vec::new();

        for chapter in chapters.clone() {
            current_group_total_words += chapter.length as f64;
            chapter_numbers.push(chapter.chapters);

            // Continue if the current group's word count exceeds the average.
            if (average_words_per_day - current_group_total_words) / average_words_per_day > tuner {
                continue;
            } else {
                datasets.push(chapter_numbers.clone());
                current_group_total_words = 0.0;
                chapter_numbers.clear();
            }
        }

        // Add any remaining chapters to the last dataset.
        if !chapter_numbers.is_empty() {
            datasets.push(chapter_numbers.clone());
        }

        // When the number of datasets matches the number of days, assign chapters to dates.
        if (datasets.len() as f64) == book_days {
            for dataset in datasets.into_iter() {
                title_chapters_days.push(ChaptersDays {
                    titles: books.titles.clone(),
                    chapters: *dataset.last().unwrap(),
                    days: current_day,
                });
                current_day += 1;
            }
            break;
        } else if (datasets.len() as f64) < book_days {
            low = tuner;
        } else {
            high = tuner;
        }
        tuner = (low + high) / 2.0;
    }
  }
  title_chapters_days
}

// Adding catch-up days to the reading plan
fn adjust_days(titles_chapters_days: Vec<ChaptersDays>, bible_data: Vec<ChapterData>, duration: i32) -> Vec<ChaptersDays> {
  let mut new_tcds: Vec<ChaptersDays> = titles_chapters_days.clone();

  // Find number of leftover days
  let last_day_value = titles_chapters_days.last().map_or(0, |last| last.days);
  let mut num_days = duration - last_day_value;

  // Add a catch-up day between the OT and NT if applicable
  if num_days > 0 {
      for i in 0..new_tcds.len() - 1 {
          let current_titles = &new_tcds[i].titles;
          let next_titles = &new_tcds[i + 1].titles;

          if current_titles.contains(&"Malachi".to_string()) && next_titles.contains(&"Matthew".to_string()) {
              insert_new_element(&mut new_tcds, i, "Catch-up day".to_string(), 0);
          }
      }
      num_days -= 1;
  }

  // Add a catch-up day at the end of the reading
  if num_days > 0 {
      let i = new_tcds.len() - 1;
      let new_element = ChaptersDays {
          titles: vec!["Catch-up day".to_string()],
          chapters: 0,
          days: new_tcds[i].days + 1,
      };
      new_tcds.push(new_element);
      num_days -= 1;
  }

  // Continue adjusting for multiple titles until there are no more leftover days
  while num_days > 1 {
      // Find elements with multiple titles
      let elements_with_multiple_titles: Vec<_> = new_tcds
          .iter()
          .filter(|entry| entry.titles.len() > 1)
          .cloned()
          .collect();

      // Find the element with the greatest number of chapters among those with multiple titles
      let max_chapters_element = elements_with_multiple_titles
          .iter()
          .max_by_key(|entry| entry.chapters);

      if let Some(max_chapters_element) = max_chapters_element {
          // Find the index of the element with the greatest chapters
          if let Some(index) = new_tcds.iter().position(|entry| *entry == *max_chapters_element) {
              // Split the element into individual elements for each title
              let titles = max_chapters_element.titles.clone();
              let num_titles = titles.len() as i32;
              let days = max_chapters_element.days;

              // Splitting N titles uses N-1 extra days; if that would overshoot, let the final
              // catch-up block handle the remainder instead.
              if num_titles - 1 > num_days {
                  break;
              }

              // Remove the original element
              new_tcds.remove(index);

              // Insert new elements for each title with adjusted days
              for (i, title) in titles.iter().enumerate() {
                  let new_element = ChaptersDays {
                      titles: vec![title.clone()],
                      chapters: bible_data.iter().find(|data| data.title == *title).unwrap().chapters,
                      days: days + i as i32,
                  };
                  new_tcds.insert(index + i, new_element);
              }

              // Adjust subsequent element days
              let adj_days = (num_titles - 1) as i32;
              for j in index + titles.len()..new_tcds.len() {
                  new_tcds[j].days += adj_days;
              }

              num_days -= num_titles;
          } else {
              // No more elements with multiple titles, break the loop
              break;
          }
      } else {
          // No more elements with multiple titles, break the loop
          break;
      }
    }

    // Prefer splitting multi-chapter days over inserting catch-up days.
    // Each split takes a single-book entry that spans multiple chapters and divides it
    // at the midpoint, consuming one leftover day.
    let last_day = new_tcds.last().map_or(0, |last| last.days);
    let mut remaining = duration - last_day;

    while remaining > 0 {
        // Find the single-title, multi-chapter entry with the largest chapter span.
        let mut prev_end: HashMap<String, i32> = HashMap::new();
        let mut best_idx: Option<usize> = None;
        let mut best_span = 1i32;

        for (i, entry) in new_tcds.iter().enumerate() {
            if entry.titles.len() != 1 || entry.chapters == 0 { continue; }
            let title = &entry.titles[0];
            let start = prev_end.get(title).copied().unwrap_or(0) + 1;
            let span = entry.chapters - start + 1;
            if span > best_span {
                best_span = span;
                best_idx = Some(i);
            }
            prev_end.insert(title.clone(), entry.chapters);
        }

        let Some(idx) = best_idx else { break };

        // Re-derive this entry's start chapter from earlier same-title entries.
        let title = new_tcds[idx].titles[0].clone();
        let mut start_chapter = 1i32;
        for entry in new_tcds[..idx].iter() {
            if entry.titles.len() == 1 && entry.titles[0] == title {
                start_chapter = entry.chapters + 1;
            }
        }

        let end_chapter = new_tcds[idx].chapters;
        let mid_chapter = (start_chapter + end_chapter) / 2;
        let day = new_tcds[idx].days;

        new_tcds[idx].chapters = mid_chapter;
        new_tcds.insert(idx + 1, ChaptersDays {
            titles: vec![title],
            chapters: end_chapter,
            days: day + 1,
        });
        for j in idx + 2..new_tcds.len() {
            new_tcds[j].days += 1;
        }
        remaining -= 1;
    }

    // Any truly remaining unassigned days become evenly-distributed catch-up days.
    let first_day = new_tcds.first().map_or(0, |first| first.days);
    let last_day = new_tcds.last().map_or(0, |last| last.days);
    let num_days = duration - last_day;
    if num_days > 0 {
      let dur = last_day - first_day;
      let days_between = (dur / (num_days + 1)) as usize;
      let mut catchup_day_count = 1usize;

      for i in 0..new_tcds.len() - 1 {
          if catchup_day_count > num_days as usize { break; }
          let current_titles = &new_tcds[i].titles;
          let next_titles = &new_tcds[i + 1].titles;

          if i > days_between * catchup_day_count && current_titles != next_titles {
              insert_new_element(&mut new_tcds, i, "Catch-up day".to_string(), 0);
              catchup_day_count += 1;
          }
      }
  }

  new_tcds
}

// Helper function to insert a new element
fn insert_new_element(new_tcds: &mut Vec<ChaptersDays>, i: usize, title: String, chapters: i32) {
  let new_element = ChaptersDays {
      titles: vec![title],
      chapters: chapters,
      days: new_tcds[i + 1].days + 1,
  };
  new_tcds.insert(i + 1, new_element);

  // Adjust subsequent element days by one day
  for j in i + 2..new_tcds.len() {
      new_tcds[j].days += 1;
  }
}

fn get_daily_reading_lengths(adjusted_plan: Vec<ChaptersDays>, chapter_data: Vec<ChapterData>) -> Vec<DailyLength> {
  let mut result: Vec<DailyLength> = Vec::new();
  let mut chapter_map: HashMap<(String, i32), i32> = HashMap::new();

  // Create a lookup map for quick access to chapter lengths
  for data in chapter_data {
      chapter_map.insert((data.title.clone(), data.chapters), data.length);
  }

  let mut prev_end_chapter = 0;
  let mut prev_title: String = String::new();

  for day in adjusted_plan {
      let mut total_length = 0;

      for title in day.titles.clone() {
          let start_chapter = if prev_title != title { 1 } else { prev_end_chapter + 1 };
          let end_chapter = day.chapters;

          // Collect lengths
          for chapter in start_chapter..=end_chapter {
              if let Some(&length) = chapter_map.get(&(title.to_string(), chapter)) {
                  total_length += length;
              }
          }

          prev_end_chapter = end_chapter;
          prev_title = title.clone();
      }

    result.push(DailyLength{ day: day.days, length: total_length});
  }

  result
}

// Get the dates for each day between the start and end dates, excluding any specified days of the week.
fn get_day_dates(start_date: NaiveDate, end_date: NaiveDate, weekdays_to_skip: &[&str]) -> Vec<NaiveDate> {
  let mut day_dates = Vec::new();
  let mut current_date = start_date;

  while current_date <= end_date {
      let day_of_week = current_date.weekday().to_string();
      if !weekdays_to_skip.iter().any(|&day| day.eq_ignore_ascii_case(&day_of_week)) {
          day_dates.push(current_date);
      }
      current_date = current_date.succ_opt().expect("Failed to get the next date");
  }

  // Ensure end_date is included if it is not skipped
  let end_day_of_week = end_date.weekday().to_string();
  if !weekdays_to_skip.iter().any(|&day| day.eq_ignore_ascii_case(&end_day_of_week)) && !day_dates.contains(&end_date) {
      day_dates.push(end_date);
  }

  day_dates
}

// Write the output file, with reading date, book(s) and chapter(s) (or 'Catch-up day' if all readings for that
// date are catch-up days)
// If length_flag: include daily reading lengths
// If duration_flag: use day count rather than dates
fn write_to_file(
  filename: &str,
  combined_plan: Vec<Vec<ChaptersDays>>,
  combined_lengths: Vec<DailyLength>,
  length_flag: bool,
  day_dates: Vec<NaiveDate>,
) -> Result<(), Box<dyn Error>> {
  let mut wtr = csv::WriterBuilder::new()
      .quote_style(csv::QuoteStyle::Never)
      .from_path(format!("{}.csv", filename))?;

  let mut prev_end_chapter = HashMap::new(); // Track the previous day's end chapter for each book

  for (i, day) in combined_plan.iter().enumerate() {
      let date_or_day = if day_dates.is_empty() {
          (i + 1).to_string()
      } else {
          day_dates[i].format("%a, %b %-d, %Y").to_string()
      };

      let books_and_chapters = day
          .iter()
          .map(|d| {
              let start_chapter = {
                  let raw = prev_end_chapter.get(&d.titles[0]).map_or(1, |&p| p + 1);
                  // If raw start exceeds this day's end chapter the book is starting a new pass; reset to 1.
                  if raw > d.chapters { 1 } else { raw }
              };

              prev_end_chapter.insert(d.titles[0].clone(), d.chapters); // Update the end chapter for the book

              if d.titles.len() > 1 {
                  // Do not state the number of chapters if there are multiple books
                  format!("\"{}\"", d.titles.join(", "))
              } else if start_chapter == d.chapters {
                  format!("\"{} {}\"", d.titles[0], d.chapters)
              } else {
                  format!("\"{} {}-{}\"", d.titles[0], start_chapter, d.chapters)
              }
          })
          .collect::<Vec<String>>()
          .join(",")
          .replace("Catch-up day 1-0", "Catch-up day");

      if length_flag {
          let length = combined_lengths
              .iter()
              .find(|&l| l.day == (i + 1) as i32)
              .map_or(0, |l| l.length);
          wtr.write_record(&[date_or_day, books_and_chapters, format!(" {}", length)])?;
      } else {
          wtr.write_record(&[date_or_day, books_and_chapters])?;
      }
  }

  wtr.flush()?;
  Ok(())
}
