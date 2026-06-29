use chrono::{NaiveDate, Datelike, Utc};
use serde::Deserialize;
use std::fs::File;
use std::error::Error;
use std::collections::HashMap;
use csv::ReaderBuilder;

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

fn main() -> Result<(), Box<dyn Error>> {
  /*
      Select the indexes of the books to read, e.g. Entire Bible 1..=66,
      OT 1..=39, NT 40..=66, Psalms & Prov 19..=20, etc.
      Multiple indexes can be included. For example, to read through the
      New Testament once and Psalms & Proverbs twice, use the following:
      vec![
          (40..=66).collect(),
          (19..=20).chain(19..=20).collect()
      ]
  */
  let book_indexes: Vec<Vec<i32>> = vec![
      (1..=39).collect(),
      (40..=66).chain(40..=66).collect()
  ];

  let mut combined_plan: Vec<Vec<ChaptersDays>> = Vec::new();
  let mut combined_lengths: Vec<DailyLength> = Vec::new();

  // Set length_flag to `true` to include daily reading lengths in the printout.
  let length_flag: bool = true;

  // OPTION 1: Select a start date, end date, and any weekdays to skip (Sun, Mon, Tue, Wed, Thu, Fri, Sat)
  let start_date = NaiveDate::from_ymd_opt(2027, 1, 1).expect("Invalid start date");
  let end_date = NaiveDate::from_ymd_opt(2028, 12, 31).expect("Invalid end date");
  let weekdays_to_skip = vec!["Fri", "Sun"]; // Days to skip (e.g. ["Sat", "Sun"])
  
  assert!(end_date > start_date, "Start date must be before end date");
  assert!(weekdays_to_skip.len() < 7, "Invalid number of weekdays to skip");
  assert!(weekdays_to_skip.iter().all(|&day| ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"].contains(&day)), "Invalid weekday to skip");
  let duration: i32;

  // OPTION 2: Use a total day count (duration) rather than selecting start and end dates. 
  // Set duration_flag to `true` and set duration value.
  let duration_flag: bool = false;

  let mut day_dates: Vec<NaiveDate> = Vec::new();
  if duration_flag {

      duration = 365; // Set the total number of days for the reading plan

      assert!(duration > 0, "Invalid duration!");
  } else {
      duration = get_duration(start_date, end_date, &weekdays_to_skip);
      day_dates = get_day_dates(start_date, end_date, &weekdays_to_skip); // Assign dates to each day
  }

  // Rename the output file if desired
  let filename = format!("reading_plan_{}", Utc::now().timestamp());

  for book_index in book_indexes {
    // Get Bible and chapter data for the selected indexes
    let bible_data: Vec<ChapterData> = get_bible_chapter_data("bible.csv", book_index.clone(), true)?;
    let chapter_data: Vec<ChapterData> = get_bible_chapter_data("bible.csv", book_index.clone(), false)?;

    // Determine a vector of the books to read and the number of days for each
    let titles_chapters_days: Vec<ChaptersDays> = get_books_in_days(bible_data.clone(), duration);
    println!("duration: {}, titles_chapters_days length: {:?}", duration, titles_chapters_days.clone().into_iter().map(|tcd| tcd.days).sum::<i32>());

    // Assign books and chapters to each day
    let titles_chapters_daily: Vec<ChaptersDays> = get_chapters_days_by_length(chapter_data.clone(), titles_chapters_days.clone(), duration);

    // Adjust the readings and fill in catch up days
    let adjusted_plan: Vec<ChaptersDays> = adjust_days(titles_chapters_daily.clone(), bible_data, duration);

    // Combine readings into a single combined plan
    for (i, day) in adjusted_plan.iter().enumerate() {
      if combined_plan.len() <= i {
          combined_plan.push(Vec::new());
      }
      combined_plan[i].push(day.clone());
    }

    // Find the daily reading lengths
    let reading_lengths: Vec<DailyLength> = get_daily_reading_lengths(adjusted_plan, chapter_data);
    for daily in reading_lengths {
      if let Some(existing) = combined_lengths.iter_mut().find(|e| e.day == daily.day) {
          existing.length += daily.length;
      } else {
          combined_lengths.push(daily);
      }
    }
  }

  // Sort the combined lengths by day
  combined_lengths.sort_by_key(|k| k.day);

  // Print the reading plan
  match write_to_file(&filename, combined_plan, combined_lengths, length_flag, day_dates) {
    Ok(_) => println!("\nSuccessfully wrote to file {}", &filename),
    Err(e) => {
        eprintln!("\nFailed to write to file: {}", e);
        std::process::exit(1);
    }
  }

  Ok(())
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
  let mut days_remaining = duration;
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
      days_remaining -= 1;
      current_day += 1;
      assert!(days_remaining >= 0, "ERROR! No remaining days to assign");
      assert!(current_day <= duration, "ERROR! The current day exceeds the total duration: {}\n", current_day);
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
                assert!(current_day <= duration,
                  "ERROR! The current day {} exceeds the total duration {}.", current_day, duration);
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
          day_dates[i].format("%a, %B %-d, %Y").to_string()
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
