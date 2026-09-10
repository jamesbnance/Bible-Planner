use chrono::{NaiveDate, Datelike, Utc};
use serde::Deserialize;
use std::fs::File;
use std::error::Error;
use std::collections::{HashMap, HashSet};
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
            if removable && ui.small_button("Remove").clicked() {
                remove = true;
            }
        });

        // Quick-select buttons — highlighted when every book in the group is selected
        ui.horizontal_wrapped(|ui| {
            for &(label, start, end) in BOOK_GROUPS {
                let active = (start..=end).all(|i| self.selected[i]);
                let fill = if active {
                    if ui.visuals().dark_mode {
                        ui.visuals().selection.bg_fill
                    } else {
                        // Light theme's default selection color (pale blue) has
                        // poor contrast with the white button text; use a
                        // darker blue instead.
                        egui::Color32::from_rgb(0, 92, 128)
                    }
                } else {
                    ui.visuals().widgets.inactive.weak_bg_fill
                };
                let text = egui::RichText::new(label)
                    .color(if active { egui::Color32::WHITE } else { ui.visuals().text_color() });
                if ui.add(egui::Button::new(text).fill(fill).small().wrap_mode(egui::TextWrapMode::Extend)).clicked() {
                    if active { self.deselect_range(start, end); } else { self.select_range(start, end); }
                }
            }

            // Styled as a plain gray outline so it reads as a distinct,
            // less-frequent action than the filled quick-select pills.
            let gray = egui::Color32::from_gray(130);
            let gray_hover = if ui.visuals().dark_mode { shade(gray, 40) } else { shade(gray, -40) };
            if outline_button(ui, "Clear", gray, gray_hover).clicked() {
                self.selected = vec![false; 66];
            }
        });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.read_times, 1, "Once");
            ui.radio_value(&mut self.read_times, 2, "Twice");
            let multi = self.read_times >= 3;
            if ui.radio(multi, "Multiple times").clicked() && !multi {
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

fn default_dark_mode() -> bool { true }
fn default_ui_scale() -> f32 { 1.2 }
fn default_reading_speed_wpm() -> u32 { 200 }
fn default_output_dir() -> String { "reading_plan".to_string() }

// Shift each RGB channel by `delta` (negative darkens, positive lightens).
fn shade(c: egui::Color32, delta: i16) -> egui::Color32 {
    let f = |v: u8| ((v as i16 + delta).clamp(0, 255)) as u8;
    egui::Color32::from_rgb(f(c.r()), f(c.g()), f(c.b()))
}

// Draw an outlined button (transparent fill, colored border + text) whose
// color shifts on hover/press. Zeroes out each state's `expansion` so the
// button doesn't grow on hover the way plain `Button::fill()` buttons do.
fn outline_button(ui: &mut egui::Ui, label: &str, base: egui::Color32, hover: egui::Color32) -> egui::Response {
    ui.scope(|ui| {
        let widgets = &mut ui.style_mut().visuals.widgets;
        for state in [&mut widgets.inactive, &mut widgets.hovered, &mut widgets.active] {
            state.expansion = 0.0;
            state.weak_bg_fill = egui::Color32::TRANSPARENT;
        }
        widgets.inactive.bg_stroke = egui::Stroke::new(1.0, base);
        widgets.inactive.fg_stroke = egui::Stroke::new(1.0, base);
        widgets.hovered.bg_stroke = egui::Stroke::new(1.0, hover);
        widgets.hovered.fg_stroke = egui::Stroke::new(1.0, hover);
        widgets.active.bg_stroke = egui::Stroke::new(1.0, hover);
        widgets.active.fg_stroke = egui::Stroke::new(1.0, hover);
        // Extend (rather than the default Wrap) so the button always requests
        // its full natural width — otherwise, in a `horizontal_wrapped` row,
        // a button placed in a narrow leftover gap shrinks and wraps its own
        // text mid-word instead of moving to the next line.
        ui.add(egui::Button::new(label).small().wrap_mode(egui::TextWrapMode::Extend))
    })
    .inner
}

// A checkbox whose checked state reads at a glance via a solid accent fill,
// matching the book quick-select buttons — the default checkbox only draws a
// thin checkmark line, which is easy to miss.
fn contrast_checkbox(ui: &mut egui::Ui, checked: &mut bool, label: &str) -> egui::Response {
    let dark_mode = ui.visuals().dark_mode;
    let is_checked = *checked;
    ui.horizontal(|ui| {
        // The box and label are drawn as two separate widgets rather than one
        // `ui.checkbox()` call: egui derives both the checkmark color and the
        // label's text color from the same style field, so recoloring the
        // checkmark to white for contrast against the accent fill would also
        // turn the label text white (invisible in light mode).
        let mut response = ui.scope(|ui| {
            if is_checked {
                let accent = if dark_mode {
                    ui.visuals().selection.bg_fill
                } else {
                    // Light theme's default selection color (pale blue) has poor
                    // contrast; use the same darker blue as the quick-select buttons.
                    egui::Color32::from_rgb(0, 92, 128)
                };
                let widgets = &mut ui.style_mut().visuals.widgets;
                for state in [&mut widgets.inactive, &mut widgets.hovered, &mut widgets.active] {
                    state.bg_fill = accent;
                    state.bg_stroke = egui::Stroke::new(1.0, accent);
                    state.fg_stroke = egui::Stroke::new(2.5, egui::Color32::WHITE);
                }
            }
            ui.add(egui::Checkbox::without_text(&mut *checked))
        })
        .inner;

        let text_response = ui.add(egui::Label::new(label).sense(egui::Sense::click()));
        if text_response.clicked() {
            *checked = !*checked;
            response.mark_changed();
        }
        response.union(text_response)
    })
    .inner
}

// Draw a solid-colored button whose fill responds to hover/press, since
// `Button::fill()` alone paints a flat color with no interaction feedback.
fn colored_button(ui: &mut egui::Ui, label: &str, base: egui::Color32, min_size: egui::Vec2) -> egui::Response {
    ui.scope(|ui| {
        let widgets = &mut ui.style_mut().visuals.widgets;
        widgets.inactive.weak_bg_fill = base;
        widgets.hovered.weak_bg_fill = shade(base, 20);
        widgets.active.weak_bg_fill = shade(base, -20);
        ui.add(
            egui::Button::new(egui::RichText::new(label).color(egui::Color32::WHITE).size(16.0))
                .min_size(min_size)
                .corner_radius(12.0),
        )
    })
    .inner
}

// A titled card used to visually group one section of the form (Books to
// Read, Schedule, Options) — an icon + bold title over a shaded panel,
// rather than a bare label floating in the page.
fn section_frame<R>(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    dark_mode: bool,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    // The surrounding canvas is tinted away from the default panel_fill (see
    // the CentralPanel setup), so using panel_fill itself here makes the card
    // read as a raised surface against it.
    let fill = ui.visuals().panel_fill;
    let stroke = egui::Stroke::new(1.0, if dark_mode { egui::Color32::from_gray(58) } else { egui::Color32::from_gray(222) });
    egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(8.0)
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            // Force the card to span the full available width even when its
            // contents (e.g. Schedule, Options) wouldn't otherwise need it,
            // so all section cards line up with the same right edge.
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(icon).size(17.0));
                ui.label(egui::RichText::new(title).size(17.0).strong().color(ui.visuals().strong_text_color()));
            });
            ui.add_space(10.0);
            add_contents(ui)
        })
        .inner
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (y, m) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    NaiveDate::from_ymd_opt(y, m, 1)
        .and_then(|d| d.pred_opt())
        .map_or(31, |d| d.day())
}

// Logical window size, in points. Kept constant across UI-scale changes by
// re-requesting it via `ViewportCommand::InnerSize` (which converts points to
// physical pixels using the current pixels_per_point) — otherwise a fixed
// physical window size would leave fewer usable points at higher scales,
// requiring the user to manually widen/heighten the window.
const WINDOW_SIZE: [f32; 2] = [560.0, 820.0];

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(WINDOW_SIZE),
        ..Default::default()
    };
    eframe::run_native(
        "Bible Reading Planner",
        options,
        Box::new(|_cc| {
            let mut app: BiblePlannerApp = std::fs::read_to_string("bible_planner_config.json")
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            // App preferences (theme, UI scale, output folder, reading speed,
            // output-format toggles, ...) persist across runs, but the plan
            // itself — track/book selections, date range, and duration —
            // always starts fresh rather than remembering the last session.
            let defaults = BiblePlannerApp::default();
            app.tracks = defaults.tracks;
            app.use_date_range = defaults.use_date_range;
            app.start_year = defaults.start_year;
            app.start_month = defaults.start_month;
            app.start_day = defaults.start_day;
            app.end_year = defaults.end_year;
            app.end_month = defaults.end_month;
            app.end_day = defaults.end_day;
            app.skip_days = defaults.skip_days;
            app.duration = defaults.duration;

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
    #[serde(default)]
    include_weekday_column: bool,
    #[serde(default)]
    include_header: bool,
    #[serde(default = "default_dark_mode")]
    dark_mode: bool,
    #[serde(default = "default_ui_scale")]
    ui_scale: f32,
    #[serde(default)]
    use_reading_minutes: bool,
    #[serde(default = "default_reading_speed_wpm")]
    reading_speed_wpm: u32,
    #[serde(default = "default_output_dir")]
    output_dir: String,
    #[serde(default)]
    export_ics: bool,
    #[serde(default)]
    extra_catchup_days: i32,
    #[serde(default)]
    catchup_after_long_books: bool,

    // Session-only state — not persisted
    #[serde(skip)]
    custom_filename: String,
    #[serde(skip)]
    last_output: Option<String>,
    #[serde(skip)]
    last_ics_output: Option<String>,
    #[serde(skip)]
    status: String,
    #[serde(skip)]
    status_is_error: bool,
    #[serde(skip)]
    status_shown_at: Option<std::time::Instant>,
    #[serde(skip)]
    confirming_reset: bool,
    #[serde(skip)]
    show_settings: bool,
    #[serde(skip)]
    show_reading_plan_dialog: bool,
    #[serde(skip)]
    show_catchup_dialog: bool,
    #[serde(skip)]
    show_output_dialog: bool,
    #[serde(skip)]
    applied_ui_scale: Option<f32>,
    // Set when generation is paused to ask the user how to handle a
    // schedule that's mostly catch-up days; holds the warning message.
    #[serde(skip)]
    pending_catchup_warning: Option<String>,
}

impl Default for BiblePlannerApp {
    fn default() -> Self {
        let mut track = ReadingTrack::empty();
        track.select_range(0, 65);

        let mut skip_days = [false; 7];
        skip_days[0] = true; // Sunday

        let next_year = Utc::now().year() + 1;

        Self {
            tracks: vec![track],
            use_date_range: true,
            start_year: next_year,
            start_month: 1,
            start_day: 1,
            end_year: next_year,
            end_month: 12,
            end_day: 31,
            skip_days,
            duration: 365,
            include_length: false,
            include_weekday_column: false,
            include_header: false,
            dark_mode: false,
            ui_scale: default_ui_scale(),
            use_reading_minutes: false,
            reading_speed_wpm: default_reading_speed_wpm(),
            output_dir: default_output_dir(),
            export_ics: false,
            extra_catchup_days: 0,
            catchup_after_long_books: false,
            custom_filename: String::new(),
            last_output: None,
            last_ics_output: None,
            status: String::new(),
            status_is_error: false,
            status_shown_at: None,
            confirming_reset: false,
            show_settings: false,
            show_reading_plan_dialog: false,
            show_catchup_dialog: false,
            show_output_dialog: false,
            applied_ui_scale: None,
            pending_catchup_warning: None,
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
        ui.ctx().set_visuals(if self.dark_mode {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        if self.applied_ui_scale != Some(self.ui_scale) {
            ui.ctx().set_pixels_per_point(self.ui_scale);
            // Re-request the same logical size so the window grows in physical
            // pixels along with the scale, keeping the same usable point-area.
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::InnerSize(WINDOW_SIZE.into()));
            self.applied_ui_scale = Some(self.ui_scale);
        }
        // The `ui` eframe hands us has no background fill, so the window would
        // otherwise show through to the raw (dark) clear color regardless of
        // theme. A CentralPanel paints a background first — tinted a bit off
        // the default panel_fill so the section cards (which use panel_fill
        // itself) read as raised surfaces against it.
        let canvas_fill = if self.dark_mode { egui::Color32::from_gray(20) } else { egui::Color32::from_gray(238) };
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(ui.style()).fill(canvas_fill).inner_margin(0))
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(20, 16))
                    .show(ui, |ui| { self.panel_contents(ui); });
            });

        let ctx = ui.ctx().clone();
        self.show_status_toast(&ctx);
        self.show_settings_dialog(&ctx);
        self.show_reading_plan_dialog(&ctx);
        self.show_catchup_dialog(&ctx);
        self.show_output_dialog(&ctx);
        self.show_catchup_warning_dialog(&ctx);
    }
}

impl BiblePlannerApp {
    fn panel_contents(&mut self, ui: &mut egui::Ui) {
        // More vertical breathing room between widgets
        ui.spacing_mut().item_spacing.y = 6.0;

        // The default scrollbar style is "floating": it overlays the content
        // rather than reserving its own space, so it sits right on top of
        // (and slightly obscures) whatever is at the right edge — the
        // settings gear, the cards, and the Generate button. A solid
        // scrollbar always reserves a lane for itself, so content shifts
        // left to make room instead of being covered.
        let mut scroll_style = egui::style::ScrollStyle::solid();
        scroll_style.bar_width = 12.0;
        // Sample the foreground (text) color instead of the pale widget
        // background fill, so the bar reads as clearly darker/higher-contrast.
        scroll_style.foreground_color = true;
        ui.style_mut().spacing.scroll = scroll_style;

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Bold, cool-toned title with a colored accent rule underneath,
            // in place of the plain default heading.
            let accent = if ui.visuals().dark_mode {
                egui::Color32::from_rgb(100, 210, 255)
            } else {
                egui::Color32::from_rgb(0, 105, 148)
            };
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 40.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.label(
                        egui::RichText::new("Bible Reading Planner")
                            .size(30.0)
                            .strong()
                            .color(accent),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let gear = egui::Button::new(egui::RichText::new("⚙").size(22.0)).frame(false);
                        if ui.add(gear).on_hover_text("Settings").clicked() {
                            self.show_settings = true;
                        }
                    });
                },
            );
            ui.add_space(6.0);
            let rule_rect = ui
                .allocate_space(egui::vec2(ui.available_width(), 3.0))
                .1;
            ui.painter().rect_filled(rule_rect, 0.0, accent);
            ui.add_space(4.0);

            // ── Books ──────────────────────────────────────────────────────
            section_frame(ui, "📖", "Books to Read", self.dark_mode, |ui| {
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
            });

            ui.add_space(10.0);

            // ── Schedule ───────────────────────────────────────────────────
            section_frame(ui, "📅", "Schedule", self.dark_mode, |ui| {
                // radio_value(&mut field, value_when_selected, "label")
                // The field is set to `value_when_selected` when this radio is clicked.
                ui.radio_value(&mut self.use_date_range, true, "Date range");
                if self.use_date_range {
                    ui.indent("date_range_fields", |ui| {
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
                                .width(50.0)
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
                                .width(50.0)
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
                    });
                    ui.add_space(4.0);
                }

                ui.radio_value(&mut self.use_date_range, false, "Fixed duration (days)");
                if !self.use_date_range {
                    ui.indent("fixed_duration_fields", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Duration:");
                            ui.add(egui::DragValue::new(&mut self.duration).range(1..=3650).suffix(" days"));
                        });
                    });
                }

            });

            ui.add_space(10.0);

            // ── Options ────────────────────────────────────────────────────
            // Each row used to hold its controls inline, which made the card
            // sprawl; now each just opens a focused dialog (see
            // show_reading_plan_dialog/show_catchup_dialog/show_output_dialog).
            section_frame(ui, "🔧", "Options", self.dark_mode, |ui| {
                let row_size = egui::vec2(ui.available_width(), 32.0);
                if ui.add_sized(row_size, egui::Button::new("Configure Bible Reading Plan...")).clicked() {
                    self.show_reading_plan_dialog = true;
                }
                ui.add_space(6.0);
                if ui.add_sized(row_size, egui::Button::new("Configure Catch-up Days...")).clicked() {
                    self.show_catchup_dialog = true;
                }
                ui.add_space(6.0);
                if ui.add_sized(row_size, egui::Button::new("Configure Output...")).clicked() {
                    self.show_output_dialog = true;
                }
            });

            ui.add_space(14.0);

            // ── Generate button ────────────────────────────────────────────
            // Same saturation/brightness as the app's blue accent (the
            // checked-checkbox/quick-select fill, rgb(0, 92, 128)) — only the
            // hue changes — so the green reads as part of the same palette
            // instead of a mismatched, separately-chosen color. Also used
            // as-is in both themes, same as that blue accent.
            let generate_fill = egui::Color32::from_rgb(0, 128, 43);
            let button_size = egui::vec2(180.0, 40.0);

            // Open Output File shares Generate Plan's row (to its left) so
            // it's visible right where the user is already looking, rather
            // than a separate row further down that's easy to miss.
            let open_fill = egui::Color32::from_rgb(0, 92, 128);
            let last_output = self.last_output.clone();
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), button_size.y),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if colored_button(ui, "Generate Plan", generate_fill, button_size).clicked() {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            self.generate_plan();
                        }));
                        if result.is_err() {
                            self.set_status("Plan generation failed — the settings caused an internal error. Try a different date range or book selection.", true);
                        }
                    }
                    if let Some(path) = &last_output {
                        ui.add_space(10.0);
                        if colored_button(ui, "📂 Open Output File", open_fill, button_size).clicked() {
                            open_file(path);
                        }
                    }
                },
            );

            // Status is shown as a centered toast (see show_status_toast) rather than inline here.
            // The toast itself (shown immediately, no scrolling needed) also
            // offers this same "open" action right after a successful generation.
            if let Some(path) = &self.last_ics_output.clone() {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if colored_button(ui, "📅 Open Calendar File", open_fill, egui::vec2(210.0, 36.0)).clicked() {
                        open_file(path);
                    }
                });
            }
        });
    }
}

// ── Planning logic called from the GUI ───────────────────────────────────────

impl BiblePlannerApp {
    fn set_status(&mut self, message: impl Into<String>, is_error: bool) {
        self.status = message.into();
        self.status_is_error = is_error;
        self.status_shown_at = Some(std::time::Instant::now());
    }

    // Status/warning toast, centered over the whole window. Auto-dismisses
    // five seconds after being shown, or immediately on click.
    fn show_status_toast(&mut self, ctx: &egui::Context) {
        if self.status.is_empty() {
            return;
        }

        const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
        let elapsed = self.status_shown_at.map_or(TIMEOUT, |t| t.elapsed());
        if elapsed >= TIMEOUT {
            self.status.clear();
            return;
        }

        let message = self.status.clone();
        let is_error = self.status_is_error;
        // Surface the "open" action here too, right when the toast appears —
        // no scrolling needed — since the standalone Open Output File button
        // further down the form is easy to miss.
        let output_path = if is_error { None } else { self.last_output.clone() };
        let mut dismissed = false;
        let mut open_requested = false;

        egui::Area::new(egui::Id::new("status_toast"))
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                const DARK_RED: egui::Color32 = egui::Color32::from_rgb(139, 0, 0);
                let (fill, stroke, text_color) = if is_error {
                    (egui::Color32::from_rgb(255, 235, 235), egui::Stroke::new(2.0, DARK_RED), DARK_RED)
                } else {
                    (
                        egui::Color32::from_rgb(46, 125, 50),
                        egui::Stroke::NONE,
                        egui::Color32::WHITE,
                    )
                };
                let response = egui::Frame::new()
                    .fill(fill)
                    .stroke(stroke)
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(18, 14))
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            let label_response = ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&message).color(text_color).strong(),
                                )
                                .sense(egui::Sense::click()),
                            );
                            if output_path.is_some() {
                                ui.add_space(8.0);
                                if outline_button(ui, "📂 Open Output File", egui::Color32::WHITE, egui::Color32::from_gray(210))
                                    .clicked()
                                {
                                    open_requested = true;
                                }
                            }
                            label_response
                        })
                        .inner
                    })
                    .inner;
                if response.clicked() {
                    dismissed = true;
                }
            });

        if open_requested {
            if let Some(path) = &output_path {
                open_file(path);
            }
            dismissed = true;
        }

        if dismissed {
            self.status.clear();
        } else {
            // Keep repainting so the timeout elapses even without further input.
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }

    // Settings dialog, shown as a modal over the rest of the app. Each
    // individual setting is separated from the next with a horizontal rule.
    fn show_settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_settings {
            return;
        }

        let mut close = false;

        let theme = if self.dark_mode { egui::Theme::Dark } else { egui::Theme::Light };
        let frame = egui::Frame::popup(&ctx.style_of(theme)).inner_margin(egui::Margin::symmetric(24, 16));
        let modal = egui::Modal::new(egui::Id::new("settings_modal")).frame(frame).show(ctx, |ui| {
            ui.set_min_width(320.0);
            ui.add_space(8.0);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 24.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.heading("⚙ Settings");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("X").clicked() {
                            close = true;
                        }
                    });
                },
            );
            ui.add_space(8.0);
            ui.separator();

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Theme:");
                ui.radio_value(&mut self.dark_mode, false, "Light");
                ui.radio_value(&mut self.dark_mode, true, "Dark");
            });
            ui.add_space(8.0);
            ui.separator();

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("UI scale:");
                ui.radio_value(&mut self.ui_scale, 1.0, "Small");
                ui.radio_value(&mut self.ui_scale, 1.2, "Medium");
                ui.radio_value(&mut self.ui_scale, 1.4, "Large");
                ui.radio_value(&mut self.ui_scale, 1.6, "Extra large");
            });
            ui.add_space(8.0);
            ui.separator();

            ui.add_space(8.0);
            if !self.confirming_reset {
                let gray = egui::Color32::from_gray(130);
                let gray_hover = if ui.visuals().dark_mode { shade(gray, 40) } else { shade(gray, -40) };
                if outline_button(ui, "Reset to Defaults", gray, gray_hover).clicked() {
                    self.confirming_reset = true;
                }
            } else {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Erase all selections and settings?").weak());
                    if ui.small_button("Confirm").clicked() {
                        *self = Self::default();
                        self.set_status("Settings reset to defaults.", false);
                        close = true;
                    }
                    if ui.small_button("Cancel").clicked() {
                        self.confirming_reset = false;
                    }
                });
            }
            ui.add_space(8.0);
        });

        if close || modal.should_close() {
            self.show_settings = false;
            self.confirming_reset = false;
        }
    }

    // Bible Reading Plan dialog: how the daily reading length is reported
    // (also holds the output's weekday/header columns, moved out of Settings
    // since they're about the same generated output, not app-wide prefs).
    fn show_reading_plan_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_reading_plan_dialog {
            return;
        }

        let mut close = false;

        let theme = if self.dark_mode { egui::Theme::Dark } else { egui::Theme::Light };
        let frame = egui::Frame::popup(&ctx.style_of(theme)).inner_margin(egui::Margin::symmetric(24, 16));
        let modal = egui::Modal::new(egui::Id::new("reading_plan_modal")).frame(frame).show(ctx, |ui| {
            ui.set_min_width(340.0);
            ui.add_space(8.0);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 24.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.heading("📏 Bible Reading Plan");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("X").clicked() {
                            close = true;
                        }
                    });
                },
            );
            ui.add_space(8.0);
            ui.separator();

            ui.add_space(8.0);
            contrast_checkbox(ui, &mut self.include_length, "Include daily reading length in output");
            if self.include_length {
                ui.add_space(8.0);
                ui.indent("reading_length_fields", |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Reading length shown as:");
                        ui.radio_value(&mut self.use_reading_minutes, false, "Word count");
                        ui.radio_value(&mut self.use_reading_minutes, true, "Minutes");
                    });
                    if self.use_reading_minutes {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label("Reading speed:");
                            ui.add(
                                egui::DragValue::new(&mut self.reading_speed_wpm)
                                    .range(50..=600)
                                    .suffix(" words/min"),
                            );
                        });
                    }
                });
            }
            ui.add_space(8.0);
            ui.separator();

            ui.add_space(8.0);
            contrast_checkbox(ui, &mut self.include_weekday_column, "Include weekday as its own column in output");
            ui.add_space(8.0);
            ui.separator();

            ui.add_space(8.0);
            contrast_checkbox(ui, &mut self.include_header, "Include column header row in output");
            ui.add_space(8.0);
        });

        if close || modal.should_close() {
            self.show_reading_plan_dialog = false;
        }
    }

    // Catch-up Days dialog: how many extra days to add and whether long
    // books automatically get one, both previously inline in Options.
    fn show_catchup_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_catchup_dialog {
            return;
        }

        let mut close = false;

        let theme = if self.dark_mode { egui::Theme::Dark } else { egui::Theme::Light };
        let frame = egui::Frame::popup(&ctx.style_of(theme)).inner_margin(egui::Margin::symmetric(24, 16));
        let modal = egui::Modal::new(egui::Id::new("catchup_modal")).frame(frame).show(ctx, |ui| {
            ui.set_min_width(340.0);
            ui.add_space(8.0);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 24.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.heading("⏱ Catch-up Days");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("X").clicked() {
                            close = true;
                        }
                    });
                },
            );
            ui.add_space(8.0);
            ui.separator();

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Extra catch-up days:");
                ui.add(egui::DragValue::new(&mut self.extra_catchup_days).range(0..=3650));
            });
            ui.label(
                egui::RichText::new("Spread evenly across the schedule; shortens the reading, not the plan.").weak(),
            );
            ui.add_space(8.0);
            ui.separator();

            ui.add_space(8.0);
            contrast_checkbox(ui, &mut self.catchup_after_long_books, "Add a catch-up day after longer books");
            ui.add_space(8.0);
        });

        if close || modal.should_close() {
            self.show_catchup_dialog = false;
        }
    }

    // Output dialog: destination folder/filename and the optional calendar
    // (.ics) export, all previously inline in Options.
    fn show_output_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_output_dialog {
            return;
        }

        let mut close = false;

        let theme = if self.dark_mode { egui::Theme::Dark } else { egui::Theme::Light };
        let frame = egui::Frame::popup(&ctx.style_of(theme)).inner_margin(egui::Margin::symmetric(24, 16));
        let modal = egui::Modal::new(egui::Id::new("output_modal")).frame(frame).show(ctx, |ui| {
            ui.set_min_width(360.0);
            ui.add_space(8.0);
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), 24.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.heading("💾 Output");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("X").clicked() {
                            close = true;
                        }
                    });
                },
            );
            ui.add_space(8.0);
            ui.separator();

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Output folder:");
                ui.label(egui::RichText::new(&self.output_dir).weak());
                if ui.small_button("Browse...").clicked() {
                    if let Some(dir) = rfd::FileDialog::new()
                        .set_directory(&self.output_dir)
                        .pick_folder()
                    {
                        self.output_dir = dir.to_string_lossy().to_string();
                    }
                }
            });
            ui.add_space(8.0);
            ui.separator();

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("Output filename:");
                // TextEdit has no visible border at rest by default (only on
                // hover/focus), so give it a constant one here.
                let border = ui.visuals().widgets.noninteractive.bg_stroke;
                let corner_radius = ui.visuals().widgets.inactive.corner_radius;
                ui.add(
                    egui::TextEdit::singleline(&mut self.custom_filename)
                        .hint_text("leave blank for auto-generated name")
                        .frame(
                            egui::Frame::new()
                                .inner_margin(egui::Margin::symmetric(4, 2))
                                .stroke(border)
                                .corner_radius(corner_radius),
                        ),
                );
            });
            ui.label(egui::RichText::new(format!(".csv is added automatically; saved under {}/", self.output_dir)).weak());
            ui.add_space(8.0);
            ui.separator();

            ui.add_space(8.0);
            ui.add_enabled_ui(self.use_date_range, |ui| {
                ui.horizontal(|ui| {
                    contrast_checkbox(ui, &mut self.export_ics, "Also export a calendar file (.ics) for Google/Apple Calendar");
                    if self.export_ics {
                        let info = ui.small_button("ℹ").on_hover_text("How to use the calendar file");
                        egui::Popup::from_toggle_button_response(&info).show(|ui| {
                            ui.set_max_width(320.0);
                            ui.label(egui::RichText::new("Using the calendar file").strong());
                            ui.add_space(4.0);
                            ui.label("The .ics file has one all-day event per reading day. Import it into your calendar app:");
                            ui.add_space(4.0);
                            ui.label("• Google Calendar (web): Settings → Import & export → Import, then choose the file.");
                            ui.label("• Apple Calendar (Mac): File → Import…, then choose the file.");
                            ui.label("• iPhone/iPad: AirDrop or email yourself the file, then tap it to add the events.");
                        });
                    }
                });
            });
            if !self.use_date_range {
                ui.label(egui::RichText::new("Calendar export needs a date range, not a fixed duration.").weak());
            }
            ui.add_space(8.0);
        });

        if close || modal.should_close() {
            self.show_output_dialog = false;
        }
    }

    // Shown instead of generating when a track's book selection has far
    // fewer chapters than the plan's duration (see `catchup_warning`), so a
    // mostly-catch-up-days plan is never produced silently.
    fn show_catchup_warning_dialog(&mut self, ctx: &egui::Context) {
        let Some(message) = self.pending_catchup_warning.clone() else {
            return;
        };

        let mut dismiss = false;
        let mut proceed = false;

        let theme = if self.dark_mode { egui::Theme::Dark } else { egui::Theme::Light };
        let frame = egui::Frame::popup(&ctx.style_of(theme)).inner_margin(egui::Margin::symmetric(24, 16));
        let modal = egui::Modal::new(egui::Id::new("catchup_warning_modal")).frame(frame).show(ctx, |ui| {
            ui.set_min_width(340.0);
            ui.add_space(8.0);
            ui.heading("Mostly Catch-up Days");
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(&message);
            ui.add_space(12.0);

            if colored_button(
                ui,
                "Include catch-up days",
                if self.dark_mode { egui::Color32::from_rgb(67, 160, 71) } else { egui::Color32::from_rgb(46, 125, 50) },
                egui::vec2(ui.available_width(), 32.0),
            ).clicked() {
                proceed = true;
            }
            ui.add_space(6.0);
            if outline_button(ui, "Re-enter schedule/duration", egui::Color32::from_gray(130), egui::Color32::from_gray(90)).clicked() {
                dismiss = true;
            }
            ui.add_space(4.0);
        });

        if proceed {
            self.pending_catchup_warning = None;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.generate_plan_impl(true);
            }));
            if result.is_err() {
                self.set_status("Plan generation failed — the settings caused an internal error. Try a different date range or book selection.", true);
            }
        } else if dismiss || modal.should_close() {
            self.pending_catchup_warning = None;
        }
    }

    // Returns a warning message if any track's selected books have far
    // fewer chapters than the plan's duration — meaning a large share of
    // the plan's days would silently become catch-up days.
    fn catchup_warning(&self, book_indexes: &[Vec<i32>], duration: i32) -> Option<String> {
        if duration <= 0 {
            return None;
        }

        let mut worst: Option<(i32, f64)> = None;
        for book_index in book_indexes {
            let Ok(bible_data) = get_bible_chapter_data("bible.csv", book_index.clone(), true) else { continue; };
            let total_chapters: i32 = bible_data.iter().map(|b| b.chapters).sum();
            if total_chapters <= 0 {
                continue;
            }
            let catchup_days = (duration - total_chapters).max(0);
            let ratio = catchup_days as f64 / duration as f64;
            if ratio >= 0.25 && worst.map_or(true, |(_, best_ratio)| ratio > best_ratio) {
                worst = Some((total_chapters, ratio));
            }
        }

        worst.map(|(chapters, _)| {
            format!(
                "The number of chapters ({}) is significantly less than the number of reading days ({}). \
                 Most days would be catch-up days with nothing new to read.",
                chapters, duration
            )
        })
    }

    fn generate_plan(&mut self) {
        self.generate_plan_impl(false);
    }

    fn generate_plan_impl(&mut self, skip_catchup_check: bool) {
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
            self.set_status("Please select at least one book.", true);
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
                None => { self.set_status("Invalid start date.", true); return; }
            };
            let end = match NaiveDate::from_ymd_opt(self.end_year, self.end_month, self.end_day) {
                Some(d) => d,
                None => { self.set_status("Invalid end date.", true); return; }
            };
            if end <= start {
                self.set_status("End date must be after start date.", true);
                return;
            }
            (get_duration(start, end, &weekdays_to_skip),
             get_day_dates(start, end, &weekdays_to_skip))
        } else {
            (self.duration, Vec::new())
        };

        if self.extra_catchup_days >= duration {
            self.set_status("Extra catch-up days must be less than the plan's total duration.", true);
            return;
        }

        if !skip_catchup_check {
            // Deliberately-requested extra catch-up days shouldn't count
            // against the "far fewer chapters than days" warning — that
            // warning is about an unintentional mismatch, not a choice the
            // user already made.
            let content_duration = (duration - self.extra_catchup_days).max(1);
            if let Some(message) = self.catchup_warning(&book_indexes, content_duration) {
                self.pending_catchup_warning = Some(message);
                return;
            }
        }

        // Run the existing planning pipeline
        let output_dir = self.output_dir.trim();
        let output_dir = if output_dir.is_empty() { "reading_plan" } else { output_dir };
        if let Err(e) = std::fs::create_dir_all(output_dir) {
            self.set_status(format!("Error creating output directory: {}", e), true);
            return;
        }
        let filename = if self.custom_filename.trim().is_empty() {
            format!("{}/reading_plan_{}", output_dir, Utc::now().timestamp())
        } else {
            let mut name = self.custom_filename.trim().to_string();
            name = name.replace(['/', '\\'], "_");
            if let Some(stripped) = name.strip_suffix(".csv") {
                name = stripped.to_string();
            }
            format!("{}/{}", output_dir, name)
        };
        let mut combined_plan: Vec<Vec<ChaptersDays>> = Vec::new();
        let mut combined_lengths: Vec<DailyLength> = Vec::new();

        for book_index in book_indexes {
            let bible_data = match get_bible_chapter_data("bible.csv", book_index.clone(), true) {
                Ok(d) => d,
                Err(e) => { self.set_status(format!("Error reading bible.csv: {}", e), true); return; }
            };
            let chapter_data = match get_bible_chapter_data("bible.csv", book_index.clone(), false) {
                Ok(d) => d,
                Err(e) => { self.set_status(format!("Error reading bible.csv: {}", e), true); return; }
            };

            let plan = build_track_plan(
                bible_data,
                chapter_data.clone(),
                duration,
                self.extra_catchup_days,
                self.catchup_after_long_books,
            );

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

        let wpm = if self.use_reading_minutes { Some(self.reading_speed_wpm) } else { None };
        let csv_path = format!("{}.csv", filename);

        let ics_result = if self.export_ics && !day_dates.is_empty() {
            Some(write_ics_file(&filename, &combined_plan, &combined_lengths, self.include_length, &day_dates, wpm))
        } else {
            None
        };

        match write_to_file(&filename, combined_plan, combined_lengths, self.include_length, day_dates, wpm, self.include_weekday_column, self.include_header) {
            Ok(_) => {
                self.last_output = Some(csv_path.clone());
                let mut message = format!("Written to {}", csv_path);
                match ics_result {
                    Some(Ok(())) => {
                        let ics_path = format!("{}.ics", filename);
                        self.last_ics_output = Some(ics_path.clone());
                        message = format!("{} and {}", message, ics_path);
                    }
                    Some(Err(e)) => {
                        self.last_ics_output = None;
                        message = format!("{} (calendar export failed: {})", message, e);
                    }
                    None => { self.last_ics_output = None; }
                }
                self.set_status(message, false);
                // Persist settings after a successful generation
                if let Ok(json) = serde_json::to_string_pretty(self) {
                    let _ = std::fs::write("bible_planner_config.json", json);
                }
            }
            Err(e) => {
                self.set_status(format!("Error: {}", e), true);
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
  // Group books into (titles, chapters, raw fractional day share) — small
  // books get combined onto a shared day, larger ones stand alone — without
  // rounding yet, so the whole set of shares can be apportioned together.
  let mut groups: Vec<(Vec<String>, i32, f64)> = Vec::new();

  // Temporary storage for book titles that will be combined into a single day's reading.
  let mut temp_titles: Vec<String> = Vec::new();
  // Accumulators for the total number of chapters from and the total number of days required for the temporary book(s).
  let mut temp_chapters: i32 = 0;
  let mut temp_days: f64 = 0.0;

  let total_chapter_count: i32 = bible_data.iter().map(|b| b.chapters).sum();
  // If duration exceeds chapter count, cap planning to chapter count; the extra days become catch-up days in adjust_days.
  let effective_duration = duration.min(total_chapter_count);

  let total_word_count: i32 = bible_data.iter().map(|b| b.length).sum();

  for book in bible_data {
      // Number of days needed to read the current book.
      let days: f64 = (book.length as f64 / total_word_count as f64) * effective_duration as f64;
      // Combine books for partial days.
      if days >= 0.66 {
          // If there are already books scheduled for the current day, finalize the day's schedule and start a new one.
          if !temp_titles.is_empty() {
              groups.push((std::mem::take(&mut temp_titles), temp_chapters, temp_days));
              temp_chapters = 0;
              temp_days = 0.0;
          }
          groups.push((vec![book.title], book.chapters, days));
      } else {
          // If the book fits within the current day, add it to the temporary storage.
          temp_titles.push(book.title);
          temp_chapters += book.chapters;
          temp_days += days;
          // If the accumulated days for the current day exceed one, finalize the day's schedule and start a new one.
          if temp_days >= 1.0 {
              groups.push((std::mem::take(&mut temp_titles), temp_chapters, temp_days));
              temp_chapters = 0;
              temp_days = 0.0;
          }
      }
  }
  // After iterating through all books, check if any remaining books must be scheduled for the last day.
  if !temp_titles.is_empty() {
      groups.push((temp_titles, temp_chapters, temp_days));
  }

  let shares: Vec<f64> = groups.iter().map(|&(_, _, days)| days).collect();
  // Groups combining multiple small books must stay on exactly one shared
  // day (get_chapters_days_by_length assumes this) — cap those at 1 so the
  // largest-remainder top-up below can never push one to 2+ days.
  let caps: Vec<i32> = groups.iter()
      .map(|(titles, _, _)| if titles.len() > 1 { 1 } else { i32::MAX })
      .collect();
  let rounded_days = apportion_days(&shares, &caps, effective_duration);

  groups.into_iter().zip(rounded_days)
      .map(|((titles, chapters, _), days)| ChaptersDays { titles, chapters, days })
      .collect()
}

// Round fractional day shares to whole days that sum to exactly `total`,
// while keeping every share at least 1 day. Rounding each share
// independently (as this used to do, floor/round based on an ad-hoc
// threshold) has no way to know about the others: many books each rounding
// up by a fraction, or many small books each floored up to the required
// minimum of 1 day, can push the *sum* of all per-book day counts above (or
// below) `total` with nothing to correct it afterward.
//
// This uses the "largest remainder" apportionment method instead: take the
// floor of each share (but never below 1, and never above that share's cap
// — used to keep a group of combined small books on exactly one shared
// day), then hand out the still-missing days one at a time to the shares
// with the largest fractional remainder — the standard way to round a set
// of shares to whole numbers that add up to an exact target. In the rare
// case where the per-share minimum of 1 alone already exceeds `total`
// (more groups than available days), a day is taken back from the smallest
// shares (down to their own minimum of 1) instead.
fn apportion_days(shares: &[f64], caps: &[i32], total: i32) -> Vec<i32> {
    let n = shares.len();
    if n == 0 {
        return Vec::new();
    }

    let mut days: Vec<i32> = shares.iter().zip(caps).map(|(&s, &cap)| (s.floor() as i32).max(1).min(cap)).collect();
    let remainder: Vec<f64> = shares.iter().zip(&days).map(|(&s, &d)| s - d as f64).collect();
    let allocated: i32 = days.iter().sum();
    let deficit = total - allocated;

    if deficit > 0 {
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| remainder[b].partial_cmp(&remainder[a]).unwrap());

        // Sweep by remainder priority, skipping any share already at its
        // cap; repeat until the deficit is used up or nothing can absorb
        // more (every share capped).
        let mut remaining = deficit;
        while remaining > 0 {
            let mut progressed = false;
            for &i in &order {
                if remaining == 0 {
                    break;
                }
                if days[i] < caps[i] {
                    days[i] += 1;
                    remaining -= 1;
                    progressed = true;
                }
            }
            if !progressed {
                break;
            }
        }
    } else if deficit < 0 {
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| shares[a].partial_cmp(&shares[b]).unwrap());
        let mut to_remove = -deficit;
        for &i in &order {
            if to_remove == 0 {
                break;
            }
            if days[i] > 1 {
                days[i] -= 1;
                to_remove -= 1;
            }
        }
    }

    days
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

    let lengths: Vec<i32> = chapters.iter().map(|c| c.length).collect();
    let group_sizes = split_into_balanced_groups(&lengths, books.days as usize);

    let mut chapter_iter = chapters.iter();
    for size in group_sizes {
        let mut last_chapter = 0;
        for _ in 0..size {
            last_chapter = chapter_iter.next().unwrap().chapters;
        }
        title_chapters_days.push(ChaptersDays {
            titles: books.titles.clone(),
            chapters: last_chapter,
            days: current_day,
        });
        current_day += 1;
    }
  }
  title_chapters_days
}

// Partition `lengths` (kept in original chapter order) into exactly
// `num_groups` contiguous groups, and return each group's chapter count.
// Chosen, in order of priority, to (1) minimize the largest group's total,
// then (2) among all splits achieving that minimum, minimize the spread
// (variance) across groups.
//
// The previous approach greedily folded chapters into an open group until
// its running total reached the book's per-day average, with no lookahead.
// That meant a chapter far larger than the average (e.g. Psalm 119, ~9x a
// typical psalm) would get merged onto whatever small chapters (e.g. 117,
// 118) were still accumulating in the open group, rather than starting its
// own day.
//
// Phase 1 finds the smallest per-day cap that still fits the chapters into
// `num_groups` days — this is what stops an oversized chapter from
// absorbing its small neighbors. But a plain greedy pack-to-cap can still
// end up with uneven groups elsewhere (e.g. some groups well under the cap
// because the next chapter would have tipped them over, others sitting
// right at it), which *raises* the spread even as it lowers the worst case.
// Phase 2 fixes that: among all ways to split into exactly `num_groups`
// groups without exceeding the cap, it uses a DP to pick the one minimizing
// the sum of squared group totals — equivalent to minimizing variance,
// since the overall total is fixed regardless of how it's split.
fn split_into_balanced_groups(lengths: &[i32], num_groups: usize) -> Vec<usize> {
    let n = lengths.len();
    let num_groups = num_groups.max(1).min(n.max(1));
    if n == 0 {
        return Vec::new();
    }
    if num_groups >= n {
        return vec![1; n];
    }

    let lens: Vec<i64> = lengths.iter().map(|&l| l as i64).collect();
    let mut prefix = vec![0i64; n + 1];
    for i in 0..n {
        prefix[i + 1] = prefix[i] + lens[i];
    }
    let range_sum = |a: usize, b: usize| prefix[b] - prefix[a]; // sum of lens[a..b]

    // Phase 1: binary search the smallest cap that fits within `num_groups`
    // contiguous groups (standard "split array, minimize the largest sum").
    let groups_needed_for_cap = |cap: i64| -> usize {
        let mut groups = 0usize;
        let mut current = 0i64;
        for &len in &lens {
            if current > 0 && current + len > cap {
                groups += 1;
                current = 0;
            }
            current += len;
        }
        if current > 0 {
            groups += 1;
        }
        groups
    };
    let mut low = *lens.iter().max().unwrap();
    let mut high: i64 = lens.iter().sum();
    while low < high {
        let mid = low + (high - low) / 2;
        if groups_needed_for_cap(mid) <= num_groups {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    let cap = low;

    // Phase 2: DP over (chapter index, group count) minimizing the sum of
    // squared group totals, restricted to groups that respect `cap`.
    // dp[i][k] = best cost to split the first i chapters into k groups;
    // choice[i][k] = the start index of the k-th (last) group.
    const UNREACHABLE: i64 = i64::MAX / 4;
    let mut dp = vec![vec![UNREACHABLE; num_groups + 1]; n + 1];
    let mut choice = vec![vec![0usize; num_groups + 1]; n + 1];
    dp[0][0] = 0;
    for i in 1..=n {
        let max_k = num_groups.min(i);
        for k in 1..=max_k {
            // j is the start of the last group (chapters j..i). As j
            // decreases, the group's sum only grows, so stop once it
            // exceeds the cap.
            for j in (k - 1..i).rev() {
                let sum = range_sum(j, i);
                if sum > cap {
                    break;
                }
                if dp[j][k - 1] == UNREACHABLE {
                    continue;
                }
                let cost = dp[j][k - 1] + sum * sum;
                if cost < dp[i][k] {
                    dp[i][k] = cost;
                    choice[i][k] = j;
                }
            }
        }
    }

    // Reconstruct group sizes by walking the choice table backwards. `cap`
    // was chosen so that an exact `num_groups`-way split always exists (a
    // single chapter alone never exceeds it), so dp[n][num_groups] is
    // always reachable here.
    let mut sizes = Vec::with_capacity(num_groups);
    let mut i = n;
    let mut k = num_groups;
    while k > 0 {
        let j = choice[i][k];
        sizes.push(i - j);
        i = j;
        k -= 1;
    }
    sizes.reverse();
    sizes
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

              // Splitting an N-title day into N single-title days only
              // consumes N-1 *extra* days (the day itself already existed).
              num_days -= num_titles - 1;
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

    // Any truly remaining unassigned days become catch-up days, spread
    // proportionally across the whole schedule rather than clustered together.
    let last_day = new_tcds.last().map_or(0, |last| last.days);
    let num_days = duration - last_day;
    if num_days > 0 {
        new_tcds = interleave_catchup_days(new_tcds, num_days);
    }

  new_tcds
}

// Interleave `num_catchup` "Catch-up day" placeholders among `entries`
// (already one per calendar day, in order), spreading them as evenly as
// possible across the combined schedule and renumbering `.days` to be
// sequential across the result.
//
// The previous approach inserted a catch-up day only at points where the
// book title changed, spaced by a fixed index gap. But inserting one always
// creates a fresh "title changed" boundary immediately after itself (a
// Catch-up day next to whatever real reading follows), which re-satisfied
// the insertion condition on the very next step — so instead of spreading
// out, catch-up days cascaded into one long unbroken run. This proportional
// interleave has no such self-triggering: it only ever compares each type's
// share of days emitted so far against its target share of the whole.
fn interleave_catchup_days(entries: Vec<ChaptersDays>, num_catchup: i32) -> Vec<ChaptersDays> {
    if num_catchup <= 0 {
        return entries;
    }

    let real_total = entries.len() as i64;
    let catchup_total = num_catchup as i64;
    let mut result = Vec::with_capacity((real_total + catchup_total) as usize);
    let mut real_iter = entries.into_iter();
    let mut real_emitted = 0i64;
    let mut catchup_emitted = 0i64;
    let mut day = 1i32;

    while real_emitted < real_total || catchup_emitted < catchup_total {
        // Emit whichever type is proportionally furthest behind its target
        // share (real_emitted/real_total vs catchup_emitted/catchup_total),
        // compared via cross-multiplication to avoid floating point.
        let take_real = if real_emitted >= real_total {
            false
        } else if catchup_emitted >= catchup_total {
            true
        } else {
            real_emitted * catchup_total <= catchup_emitted * real_total
        };

        if take_real {
            let mut entry = real_iter.next().expect("real_emitted < real_total");
            entry.days = day;
            result.push(entry);
            real_emitted += 1;
        } else {
            result.push(ChaptersDays { titles: vec!["Catch-up day".to_string()], chapters: 0, days: day });
            catchup_emitted += 1;
        }
        day += 1;
    }

    result
}

// Helper function to insert a new element
fn insert_new_element(new_tcds: &mut Vec<ChaptersDays>, i: usize, title: String, chapters: i32) {
  let new_element = ChaptersDays {
      titles: vec![title],
      // Take over the day slot right after `i` — the entry that used to sit
      // there shifts to the next day via the loop below. Using `+ 1` here
      // (as if the new entry went *after* that day too) skipped a day
      // number entirely and left both entries on the same day.
      chapters: chapters,
      days: new_tcds[i + 1].days,
  };
  new_tcds.insert(i + 1, new_element);

  // Adjust subsequent element days by one day
  for j in i + 2..new_tcds.len() {
      new_tcds[j].days += 1;
  }
}

// A book that would take at least this many days to read on its own counts
// as "long" for the optional catch-up-day-after-long-books feature.
const LONG_BOOK_DAY_THRESHOLD: i32 = 5;

// Build one track's day-by-day plan, optionally reserving some days as
// catch-up days — either a user-requested count spread evenly across the
// whole schedule, or one placed right after each completed run of a book
// long enough to cross LONG_BOOK_DAY_THRESHOLD. Both kinds of extra
// catch-up day displace reading days rather than extending the plan: the
// real content is squeezed into `duration` minus however many catch-up days
// are needed, so the final result is still exactly `duration` days long.
fn build_track_plan(
    bible_data: Vec<ChapterData>,
    chapter_data: Vec<ChapterData>,
    duration: i32,
    extra_catchup_days: i32,
    catchup_after_long_books: bool,
) -> Vec<ChaptersDays> {
    let extra_catchup_days = extra_catchup_days.max(0);
    let target_after_extra = (duration - extra_catchup_days).max(1);

    // Identify which book titles are long enough to warrant a catch-up day
    // after each finished run, using a first pass against the full
    // duration — shaving off a handful of days for catch-up essentially
    // never changes which books cross the threshold.
    let long_book_titles: HashSet<String> = if catchup_after_long_books {
        get_books_in_days(bible_data.clone(), duration)
            .iter()
            .filter(|d| d.titles.len() == 1 && d.days >= LONG_BOOK_DAY_THRESHOLD)
            .map(|d| d.titles[0].clone())
            .collect()
    } else {
        HashSet::new()
    };

    // How many days to spend on real content, leaving the rest for
    // long-book catch-up days. Guessing this from a single probe pass isn't
    // exact — shrinking the duration to make room for catch-up days can
    // itself shift exactly where a book's run ends relative to the next, in
    // or out of counting as a transition — so this re-measures the actual
    // count each time and nudges the guess until it converges, rather than
    // trusting the first estimate.
    let mut content_duration = target_after_extra;
    let mut plan = Vec::new();
    for _ in 0..6 {
        let tcd = get_books_in_days(bible_data.clone(), content_duration);
        let tcd2 = get_chapters_days_by_length(chapter_data.clone(), tcd, content_duration);
        let mut candidate = adjust_days(tcd2, bible_data.clone(), content_duration);

        if !long_book_titles.is_empty() {
            for &i in long_book_transitions(&candidate, &long_book_titles).iter().rev() {
                insert_new_element(&mut candidate, i, "Catch-up day".to_string(), 0);
            }
        }

        let overshoot = candidate.len() as i32 - target_after_extra;
        plan = candidate;
        if overshoot == 0 || content_duration <= 1 {
            break;
        }
        content_duration = (content_duration - overshoot).max(1);
    }

    // Unconditional safety net: whatever the loop above converged to (or
    // didn't, within its iteration budget), the result must still end up at
    // exactly `duration` days — pad any shortfall, and if it's somehow
    // still over, drop trailing catch-up placeholders (never real content)
    // before falling back to a hard truncate as an absolute last resort.
    let shortfall = duration - plan.len() as i32;
    if shortfall > 0 {
        plan = interleave_catchup_days(plan, shortfall);
    } else if shortfall < 0 {
        let mut to_remove = -shortfall;
        while to_remove > 0 && plan.last().is_some_and(|d| d.chapters == 0) {
            plan.pop();
            to_remove -= 1;
        }
        if plan.len() as i32 > duration {
            plan.truncate(duration.max(0) as usize);
        }
    }

    plan
}

// Index of every entry in `entries` that's the last day of a run of a
// single title in `titles`, i.e. a point where a catch-up day belongs right
// after. Excludes the very end of `entries` — there's nothing left to catch
// up before.
fn long_book_transitions(entries: &[ChaptersDays], titles: &HashSet<String>) -> Vec<usize> {
    entries.iter().enumerate()
        .filter(|(i, day)| {
            day.titles.len() == 1
                && titles.contains(&day.titles[0])
                && entries.get(i + 1).is_some_and(|next| next.titles != day.titles)
        })
        .map(|(i, _)| i)
        .collect()
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

      // Catch-up days (chapters == 0) have no reading of their own and must
      // not touch prev_title/prev_end_chapter — otherwise the next real day
      // for a book that was already in progress looks like a fresh start
      // (prev_title would be "Catch-up day", not the book's actual title),
      // resetting its start chapter back to 1 and summing every chapter
      // from the beginning of the book instead of just the new ones.
      if day.chapters > 0 {
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
// date are catch-up days). Each active reading track gets its own column, since combined_plan[i]
// holds one entry per track for that day.
// If length_flag: include daily reading lengths
// If duration_flag: use day count rather than dates
// If wpm is Some, the length column is estimated reading minutes instead of a raw word count
// If weekday_flag: put the weekday in its own column, first (date-range mode only)
// If header_flag: write a header row matching the columns actually present (optional weekday,
// date/day, one column per track, optional word/minute count)
fn write_to_file(
  filename: &str,
  combined_plan: Vec<Vec<ChaptersDays>>,
  combined_lengths: Vec<DailyLength>,
  length_flag: bool,
  day_dates: Vec<NaiveDate>,
  wpm: Option<u32>,
  weekday_flag: bool,
  header_flag: bool,
) -> Result<(), Box<dyn Error>> {
  // Default QuoteStyle::Necessary: any field containing a comma, quote, or
  // newline (e.g. "Genesis, Exodus" for a multi-book day) is auto-quoted and
  // escaped, so it can never be misread as extra columns.
  let mut wtr = csv::WriterBuilder::new().from_path(format!("{}.csv", filename))?;

  let has_weekday_column = weekday_flag && !day_dates.is_empty();
  // Each track contributes one entry per day, so this is the number of
  // "reading" columns the data rows will actually have.
  let track_count = combined_plan.iter().map(|day| day.len()).max().unwrap_or(1).max(1);

  if header_flag {
      let mut header: Vec<String> = Vec::new();
      if has_weekday_column {
          header.push("Weekday".to_string());
      }
      header.push(if day_dates.is_empty() { "Day".to_string() } else { "Date".to_string() });
      if track_count <= 1 {
          header.push("Reading".to_string());
      } else {
          header.extend((1..=track_count).map(|t| format!("Track {}", t)));
      }
      if length_flag {
          header.push(if wpm.is_some() { "Minutes".to_string() } else { "Words".to_string() });
      }
      wtr.write_record(&header)?;
  }

  let mut prev_end_chapter = HashMap::new(); // Track the previous day's end chapter for each book

  for (i, day) in combined_plan.iter().enumerate() {
      let mut record: Vec<String> = Vec::new();

      if day_dates.is_empty() {
          record.push((i + 1).to_string());
      } else if has_weekday_column {
          record.push(day_dates[i].format("%a").to_string());
          record.push(day_dates[i].format("%b %-d %Y").to_string());
      } else {
          record.push(day_dates[i].format("%a %b %-d %Y").to_string());
      }

      for d in day.iter() {
          let start_chapter = {
              let raw = prev_end_chapter.get(&d.titles[0]).map_or(1, |&p| p + 1);
              // If raw start exceeds this day's end chapter the book is starting a new pass; reset to 1.
              if raw > d.chapters { 1 } else { raw }
          };

          prev_end_chapter.insert(d.titles[0].clone(), d.chapters); // Update the end chapter for the book

          let entry = if d.titles.len() > 1 {
              // Do not state the number of chapters if there are multiple books
              d.titles.join(", ")
          } else if start_chapter == d.chapters {
              format!("{} {}", d.titles[0], d.chapters)
          } else if d.chapters == 0 {
              "Catch-up day".to_string()
          } else {
              format!("{} {}-{}", d.titles[0], start_chapter, d.chapters)
          };

          record.push(entry);
      }

      if length_flag {
          let length = combined_lengths
              .iter()
              .find(|&l| l.day == (i + 1) as i32)
              .map_or(0, |l| l.length);
          // The `length` field in the source data is actually a character
          // count, not a word count. Approximate word count by dividing by
          // the average English word length (5 characters).
          let word_count = length as f64 / 5.0;
          let value = match wpm {
              Some(w) if w > 0 => (word_count / w as f64).round() as i32,
              _ => word_count.round() as i32,
          };
          record.push(format!(" {}", value));
      }

      wtr.write_record(&record)?;
  }

  wtr.flush()?;
  Ok(())
}

// Escape text per RFC 5545 (backslash, comma, semicolon, newline).
fn escape_ics_text(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

// Write an iCalendar (.ics) file with one all-day event per reading day, so
// the plan can be imported directly into Google Calendar or iPhone/Apple
// Calendar. Only meaningful when day_dates is non-empty (date-range mode).
fn write_ics_file(
    filename: &str,
    combined_plan: &[Vec<ChaptersDays>],
    combined_lengths: &[DailyLength],
    length_flag: bool,
    day_dates: &[NaiveDate],
    wpm: Option<u32>,
) -> Result<(), Box<dyn Error>> {
    let mut ics = String::new();
    let mut line = |s: &str| { ics.push_str(s); ics.push_str("\r\n"); };

    line("BEGIN:VCALENDAR");
    line("VERSION:2.0");
    line("PRODID:-//Better Bible Planner//EN");
    line("CALSCALE:GREGORIAN");

    let dtstamp = Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let mut prev_end_chapter: HashMap<String, i32> = HashMap::new();

    for (i, day) in combined_plan.iter().enumerate() {
        if i >= day_dates.len() { break; }
        let date = day_dates[i];

        let mut summary = day
            .iter()
            .map(|d| {
                let start_chapter = {
                    let raw = prev_end_chapter.get(&d.titles[0]).map_or(1, |&p| p + 1);
                    if raw > d.chapters { 1 } else { raw }
                };
                prev_end_chapter.insert(d.titles[0].clone(), d.chapters);

                if d.titles.len() > 1 {
                    d.titles.join(", ")
                } else if start_chapter == d.chapters {
                    format!("{} {}", d.titles[0], d.chapters)
                } else {
                    format!("{} {}-{}", d.titles[0], start_chapter, d.chapters)
                }
            })
            .collect::<Vec<String>>()
            .join(", ")
            .replace("Catch-up day 1-0", "Catch-up day");

        if length_flag {
            let length = combined_lengths
                .iter()
                .find(|&l| l.day == (i + 1) as i32)
                .map_or(0, |l| l.length);
            // The `length` field in the source data is actually a character
            // count, not a word count. Approximate word count by dividing by
            // the average English word length (5 characters).
            let word_count = length as f64 / 5.0;
            let value = match wpm {
                Some(w) if w > 0 => (word_count / w as f64).round() as i32,
                _ => word_count.round() as i32,
            };
            let unit = if wpm.is_some() { "min" } else { "words" };
            summary = format!("{} ({} {})", summary, value, unit);
        }

        let dtstart = date.format("%Y%m%d").to_string();
        let dtend = (date + chrono::Duration::days(1)).format("%Y%m%d").to_string();

        line("BEGIN:VEVENT");
        line(&format!("UID:bbp-{}-{}@betterbibleplanner", dtstart, i));
        line(&format!("DTSTAMP:{}", dtstamp));
        line(&format!("DTSTART;VALUE=DATE:{}", dtstart));
        line(&format!("DTEND;VALUE=DATE:{}", dtend));
        line(&format!("SUMMARY:{}", escape_ics_text(&summary)));
        line("END:VEVENT");
    }

    line("END:VCALENDAR");

    std::fs::write(format!("{}.ics", filename), ics)?;
    Ok(())
}
