use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::error::Error;
use std::collections::HashMap;
use chrono::{ Duration, NaiveDate, Utc };
use csv::ReaderBuilder;
use serde::Deserialize;

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

#[derive(Debug, Deserialize, Clone)]
struct ChaptersDays {
    pub titles: Vec<String>,
    pub chapters: i32,
    pub days: i32
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
struct ChaptersDate {
    pub titles: Vec<String>,
    pub chapters: i32,
    pub date: NaiveDate
}

#[derive(Debug, Deserialize, Clone)]
struct DailyLength {
    pub date: NaiveDate,
    pub length: i32
}

fn main() -> Result<(), Box<dyn Error>> {

    // Entire Bible 1..=66, OT 1..=39, NT 40..=66, Psalms & Prov 19..=20
    let book_indexes: Vec<Vec<i32>> = vec![
        // Read the New Testament, and twice through Psalms and Proverbs
        (40..=66).collect(),
        (19..=20).chain(19..=20).collect(),
    ];

    // Set the dates for reading and get the reading duration in days
    let start_date = NaiveDate::from_ymd_opt(2025, 6, 21).expect("Invalid date");
    let end_date = NaiveDate::from_ymd_opt(2025, 9, 21).expect("Invalid date");

    assert!(end_date > start_date, "Invalid dates!");
    let duration: i32 = get_duration(start_date, end_date);

    let filename = format!("reading_plan_{}", Utc::now().timestamp());

    let mut combined_plans: Vec<Vec<ChaptersDate>> = Vec::new();
    let mut combined_lengths_map: HashMap<NaiveDate, i32> = HashMap::new();

    for book_index in book_indexes {
        // Get Bible and chapter data for the selected indexes
        let bible_data: Vec<ChapterData> = get_data_combined("bible.csv", book_index.clone(), true)?;
        let chapter_data: Vec<ChapterData> = get_data_combined("bible.csv", book_index.clone(), false)?;

        // Determine a vector of the books to read and the number of days for each
        let titles_chapters_days: Vec<ChaptersDays> = get_books_in_days(bible_data.clone(), duration);

        // Assign books and chapters to dates
        let titles_chapters_date: Vec<ChaptersDate> = get_chapters_dates_by_length(chapter_data.clone(), titles_chapters_days, start_date, end_date);

        // Adjust dates and fill in catch-up days
        let adjusted_plan: Vec<ChaptersDate> = adjust_dates(titles_chapters_date, bible_data, end_date);

        // Combine this adjusted plan into the combined_plans
        for (i, chapter_date) in adjusted_plan.clone().into_iter().enumerate() {
            if combined_plans.len() <= i {
                combined_plans.push(vec![]);
            }
            combined_plans[i].push(chapter_date);
        }

        // Find the daily reading lengths
        let reading_lengths: Vec<DailyLength> = get_daily_reading_lengths(adjusted_plan, chapter_data);

        // Combine the reading lengths
        for daily in reading_lengths.clone().into_iter() {
            combined_lengths_map
                .entry(daily.date)
                .and_modify(|e| *e += daily.length)
                .or_insert(daily.length);
        }
    }

    // Convert the HashMap to a Vec<DailyLength> and sort by date
    let mut combined_lengths: Vec<DailyLength> = combined_lengths_map
        .into_iter()
        .map(|(date, length)| DailyLength { date, length })
        .collect();

    combined_lengths.sort_by_key(|k| k.date);

    // Write final plan to file
    match write_to_file(&filename, combined_plans, combined_lengths, false) {
        Ok(_) => println!("\nSuccessfully wrote to file {}", &filename),
        Err(e) => {
            eprintln!("\nFailed to write to file: {}", e);
            std::process::exit(1);
        }
    }
    Ok(())
}

// Find duration in days
fn get_duration(start: NaiveDate, end: NaiveDate) -> i32 {
    let duration_in_hms = end.and_hms_opt(0, 0, 0).unwrap() - start.and_hms_opt(0, 0, 0).unwrap();
    duration_in_hms.num_days() as i32
}

// Create a vector with title, number of chapters, total length
fn get_data_combined(file_path: &str, book_index: Vec<i32>, accumulate: bool) -> Result<Vec<ChapterData>, Box<dyn Error>> {
    let mut data: Vec<ChapterData> = Vec::new();

    for index in book_index {
        // Re-open the CSV file and reinitialize the reader to start from the beginning
        let file = File::open(file_path)?;
        let mut rdr = ReaderBuilder::new().has_headers(true).from_reader(file);

        // Optionally use a HashMap to accumulate data when aggregation is required
        let mut book_map: HashMap<String, ChapterData> = HashMap::new();

        // Iterate over the CSV records
        for result in rdr.deserialize() {
            let record: IndexData = result?;

            // Check if the current book's index matches the current book_index
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
    if duration > total_chapter_count {
        panic!("ERROR! The number of days may not exceed the number of chapters: {} > {}\n",
            duration, total_chapter_count
        );
    }

    let total_word_count: i32 = bible_data.iter().map(|b| b.length).sum();

    for book in bible_data {
        // Number of days needed to read the current book.
        let days: f32 = (book.length as f32 / total_word_count as f32) * duration as f32;
        // Combine books for partial days.
        if days >= 0.66 {
            // If there are already books scheduled for the current day, finalize the day's schedule and start a new one.
            if !temp_titles.is_empty() {
                push_new_element(&mut result, temp_titles, temp_chapters, temp_days, duration);
                temp_titles = Vec::new();
                temp_chapters = 0;
                temp_days = 0.0;
            }
            push_new_element(&mut result, vec![book.title], book.chapters, days, duration);
        } else {
            // If the book fits within the current day, add it to the temporary storage.
            temp_titles.push(book.title);
            temp_chapters += book.chapters;
            temp_days += days as f32;
            // If the accumulated days for the current day exceed one, finalize the day's schedule and start a new one.
            if temp_days >= 1.0 {
                push_new_element(&mut result, temp_titles, temp_chapters, temp_days, duration);
                temp_titles = Vec::new();
                temp_chapters = 0;
                temp_days = 0.0;
            }
        }
    }
    // After iterating through all books, check if any remaining books must be scheduled for the last day.
    if !temp_titles.is_empty() {
        push_new_element(&mut result, temp_titles, temp_chapters, temp_days, duration);
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

// Assign books and chapters to dates, taking into account chapter lengths
fn get_chapters_dates_by_length(chapter_data: Vec<ChapterData>, titles_chapters_days: Vec<ChaptersDays>, start: NaiveDate, end: NaiveDate) -> Vec<ChaptersDate> {
    let mut title_chapters_dates: Vec<ChaptersDate> = Vec::new();
    let mut current_date: NaiveDate = start;

    // Iterate through each set of books and chapters grouped by days
    for books in titles_chapters_days {
        if books.chapters < books.days {
            panic!("\nThe number of chapters in {} is less than the number of days assigned: {} < {}.\nAdd more chapters or reduce the number of days.\n",
                books.titles[0], books.chapters, books.days);
        }
        // If exactly one day is assigned, directly assign the book to the current date.
        if books.days == 1 {
            title_chapters_dates.push(ChaptersDate {
                titles: books.titles,
                chapters: books.chapters,
                date: current_date
            });
            // Move to the next date and ensure the date does not exceed the end date.
            current_date = current_date.succ_opt().unwrap();
            assert!(current_date <= end, "Reading dates go past last designated date!");
            continue;
        }
        assert!(books.titles.len() == 1, "Multiple books for more than 1 day");

        // Load the data for the particular book into chapters
        let title = &books.titles[0];
        let book_days: f64 = books.days as f64;
        let mut chapters: Vec<ChapterData> = Vec::new();
        for data in chapter_data.clone() {
            if &data.title == title {
                chapters.push(data.clone());
            }
            // Stop loading data once the book's last chapter is reached.
            if &data.title == title && data.chapters == books.chapters
            {
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
                    title_chapters_dates.push(ChaptersDate {
                        titles: books.titles.clone(),
                        chapters: *dataset.last().unwrap(),
                        date: current_date,
                    });
                    current_date = current_date.succ_opt().unwrap();
                    assert!(current_date <= end, "Reading dates go past last designated date!");
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
    title_chapters_dates
}

// Adjust dates, fill in catch-up days, split up combined readings if reasonable
fn adjust_dates(titles_chapters_date: Vec<ChaptersDate>, bible_data: Vec<ChapterData>, end: NaiveDate) -> Vec<ChaptersDate> {
    let mut new_tcds: Vec<ChaptersDate> = titles_chapters_date.clone();

    // find initial number of leftover days
    let last_date: NaiveDate = if let Some(ChaptersDate { date, .. }) = titles_chapters_date.last() {
        *date
    } else { end };
    let diff = end - last_date;
    let mut num_days = diff.num_days();

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
        // Insert a new element
        let new_date = new_tcds[i].date + Duration::days(1);
        let new_element = ChaptersDate { titles: vec!["Catch-up day".to_string()], chapters: 0, date: new_date };
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
            if let Some(index) = new_tcds.iter().position(| entry| *entry == *max_chapters_element) {
                // Split the element into individual elements for each title
                let titles = max_chapters_element.titles.clone();
                let num_titles = titles.len() as i32;
                let date = max_chapters_element.date;

                // Remove the original element
                new_tcds.remove(index);
    
                // Insert new elements for each title with adjusted dates
                for (i, title) in titles.iter().enumerate() {
                    let new_date = date + Duration::days(i as i64);
                    let new_element = ChaptersDate {
                        titles: vec![title.clone()],
                        chapters: bible_data.iter().find(|data| data.title == *title).unwrap().chapters,
                        date: new_date,
                    };
                    new_tcds.insert(index + i, new_element);
                }

                // Adjust subsequent element dates
                let adj_days = (num_titles - 1) as i64;
                for j in index + titles.len()..new_tcds.len() {
                    new_tcds[j].date = new_tcds[j].date + Duration::days(adj_days);
                }

                num_days -= num_titles as i64;
            } else {
                // No more elements with multiple titles, break the loop
                break;
            }
        } else {
            // No more elements with multiple titles, break the loop
            break;
        }
    }

    // If any days remain unassigned, add catch-up days regularly throughout
    let first_date: NaiveDate = if let Some(ChaptersDate { date, .. }) = new_tcds.first() {
        *date
    } else { end };
    let last_date: NaiveDate = if let Some(ChaptersDate { date, .. }) = new_tcds.last() {
        *date
    } else { end };
    
    let num_days = (end - last_date).num_days();
    if num_days > 0 {
        let dur = (last_date - first_date).num_days();
        let days_between = (dur / (num_days + 1)) as usize;
        let mut catchup_day_count = 1;

        for i in 0..new_tcds.len() - 1 {
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

// Used in the adjust_dates function
fn insert_new_element(new_tcds: &mut Vec<ChaptersDate>, i: usize, title: String, chapters: i32) {
    // Insert a new element
    let new_date = new_tcds[i + 1].date;
    let new_element = ChaptersDate { 
        titles: vec![title], 
        chapters: chapters, 
        date: new_date 
    };
    new_tcds.insert(i + 1, new_element);

    // Adjust subsequent element dates by one day
    for j in i + 2..new_tcds.len() {
        new_tcds[j].date = new_tcds[j].date + Duration::days(1);
    }
}

fn get_daily_reading_lengths(adjusted_plan: Vec<ChaptersDate>, chapter_data: Vec<ChapterData>) -> Vec<DailyLength> {
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

        result.push(DailyLength{ date: day.date, length: total_length});
    }

    result
}

// Write the output file: filling in start days, and writing 'Catch-up day' only if all readings
// for that date are catch-up days; otherwise include only the readings that are book and chapters
fn write_to_file(filename: &str, combined_plans: Vec<Vec<ChaptersDate>>,
    combined_lengths: Vec<DailyLength>, length_flag: bool) -> std::io::Result<()> {
    let mut file_path = PathBuf::from(env::current_dir()?);
    file_path.push(filename);
    let mut file = File::create(file_path)?;

    // HashMap to keep track of the last chapter read for each book
    let mut last_chapters: HashMap<String, i32> = HashMap::new();

    // Iterate through each date's plans, accumulating output for the date's readings and
    // determining if the date is a catch-up day, then write the output to the file
    for (date_plans, daily_length) in combined_plans.into_iter().zip(combined_lengths.into_iter()) {
        let date = date_plans[0].date;
        let mut output = String::new();
        let mut is_catch_up_day = true;

        // Process each plan for the current date
        for plan in &date_plans {
            let titles = plan.titles.join(", ");
            if titles == "Catch-up day" {
                continue;
            } else {
                is_catch_up_day = false;
                // Update the last chapter read for the current book
                let last_chapter = last_chapters.entry(titles.clone()).or_insert(0);
                // Determine the starting chapter for the current plan
                let mut start_chapter = if *last_chapter == 0 { 1 } else { *last_chapter + 1 };
                let chapters = if start_chapter == plan.chapters {
                    format!("{}", plan.chapters)
                } else {
                    start_chapter = if start_chapter > plan.chapters { 1 } else { start_chapter };
                    format!("{}-{}", start_chapter, plan.chapters)
                };

                output.push_str(&format!("{} {}, ", titles, chapters));
                *last_chapter = plan.chapters;
            }
        }

        // If the current date is marked as a catch-up day, write it to the file
        if is_catch_up_day {
            writeln!(file, "{} Catch-up day", date.format("%b %e, %Y"))?;
        } else {
            // Otherwise, write the accumulated output for the current date to the file
            output.pop(); // Remove the trailing comma and space
            output.pop();

            // If length_flag is true, include the length of the reading for the day
            if length_flag {
                writeln!(
                    file,
                    "{}  {} ({})",
                    date.format("%b %e, %Y"),
                    output,
                    daily_length.length
                )?;
            } else {
                writeln!(file, "{}  {}", date.format("%b %e, %Y"), output)?;
            }
        }
    }

    Ok(())
}
