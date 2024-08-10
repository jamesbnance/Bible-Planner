use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::error::Error;
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

fn main() -> Result<(), Box<dyn Error>> {

    // Entire Bible 1..=66, OT 1..=39, NT 40..=66, Psalms & Prov 19..=20
    let book_index: Vec<i32> = (1..=66).collect();

    // Set the dates for reading and get the reading duration in days
    let start_date = NaiveDate::from_ymd_opt(2025, 1, 31).unwrap();
    let end_date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();

    assert!(end_date > start_date, "Invalid dates!");
    let duration: i32 = get_duration(start_date, end_date);

    let filename = format!("reading_plan_{}", Utc::now().timestamp());

    let bible_data: Vec<ChapterData> = get_data_combined("bible.csv", book_index.clone(), true)?;
    let chapter_data: Vec<ChapterData> = get_data_combined("bible.csv", book_index.clone(), false)?;

    // Determine a vector of the books to read and the number of days for each
    let titles_chapters_days: Vec<ChaptersDays> = get_books_in_days(bible_data.clone(), duration);

    // Assign books and chapters to dates
    let titles_chapters_date: Vec<ChaptersDate> = get_chapters_dates_by_length(chapter_data, titles_chapters_days, start_date, end_date);

    // Adjust dates and fill in catch-up days
    let adjusted_plan: Vec<ChaptersDate> = adjust_dates(titles_chapters_date, bible_data, end_date);

    // Write final plan to file
    match write_to_file(&filename, adjusted_plan) {
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
    let bible_file = File::open(file_path)?;
    let mut reader = ReaderBuilder::new().from_reader(bible_file);
    let mut data_vec: Vec<ChapterData> = Vec::new();

    for result in reader.deserialize() {
        let record: IndexData = result?;
        if book_index.contains(&record.index) {
            if accumulate {
                let mut found = false;
                for data in &mut data_vec {
                    if data.title == record.title {
                        data.chapters += 1;
                        data.length += record.length;
                        found = true;
                        break;
                    }
                }
                if !found {
                    data_vec.push(ChapterData {
                        title: record.title.clone(),
                        chapters: 1,
                        length: record.length,
                    });
                }
            } else {
                data_vec.push(ChapterData {
                    title: record.title,
                    chapters: record.chapter,
                    length: record.length,
                });
            }
        }
    }

    Ok(data_vec)
}

// Determine a vector of the books to read and the number of days for each
fn get_books_in_days(bible_data: Vec<ChapterData>, duration: i32) -> Vec<ChaptersDays> {
    let mut result = Vec::new();
    let mut temp_titles: Vec<String> = Vec::new();
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
        let days: f32 = (book.length as f32 / total_word_count as f32) * duration as f32;
        // combine books for partial days
        if days >= 0.66 {
            if !temp_titles.is_empty() {
                push_new_element(&mut result, temp_titles, temp_chapters, temp_days, duration);
                temp_titles = Vec::new();
                temp_chapters = 0;
                temp_days = 0.0;
            }
            push_new_element(&mut result, vec![book.title], book.chapters, days, duration);
        } else {
            temp_titles.push(book.title);
            temp_chapters += book.chapters;
            temp_days += days as f32;
            if temp_days >= 1.0 {
                push_new_element(&mut result, temp_titles, temp_chapters, temp_days, duration);
                temp_titles = Vec::new();
                temp_chapters = 0;
                temp_days = 0.0;
            }
        }
    }
    if !temp_titles.is_empty() {
        push_new_element(&mut result, temp_titles, temp_chapters, temp_days, duration);
    }
    result
}

fn push_new_element(result: &mut Vec<ChaptersDays>, titles: Vec<String>, chapters: i32, days: f32, duration: i32) {
    let new_element = ChaptersDays {titles, chapters, days: round_days(days, duration)};
    result.push(new_element);
}

// Round down for a relatively large number of days, otherwise round to nearest whole
fn round_days(days: f32, duration: i32) -> i32 {
    let rdays = duration as f32 / 30.0;
    let mut rdays = if days > rdays {
        days as i32
    } else {
        days.round() as i32
    };
    if rdays == 0 {
        rdays = 1;
    }
    rdays
}

// Assign books and chapters to dates, taking into account chapter lengths
fn get_chapters_dates_by_length(chapter_data: Vec<ChapterData>, tcds: Vec<ChaptersDays>, start: NaiveDate, end: NaiveDate) -> Vec<ChaptersDate> {
    let mut title_chapters_dates: Vec<ChaptersDate> = Vec::new();
    let mut current_date: NaiveDate = start;
    for books in tcds {
        if books.days == 1 {
            title_chapters_dates.push(ChaptersDate {
                titles: books.titles,
                chapters: books.chapters,
                date: current_date
            });
            current_date = current_date.succ_opt().unwrap();
            assert!(current_date <= end, "Reading dates go past last designated date!");
            continue;
        }
        assert!(books.titles.len() == 1, "Multiple books for more than 1 day");
        let title = books.titles.first();
        let n: f64 = books.days as f64;
        let chapters = chapter_data.clone().into_iter().filter(| t | Some(&t.title) == title);
        let total_words: f64 = chapters.clone().map(|chapter| chapter.length as f64).sum();
        let average_words_per_day: f64 = total_words / n;

        let mut low = 0.0;
        let mut high = 1.0;
        let mut tuner = 0.0;
        loop {
            let mut datasets: Vec<Vec<i32>> = Vec::new();
            let mut current_group_total_words: f64 = 0.0;
            let mut chapter_numbers: Vec<i32> = Vec::new();

            for chapter in chapters.clone() {
                current_group_total_words += chapter.length as f64;
                chapter_numbers.push(chapter.chapters);

                if (average_words_per_day - current_group_total_words) / average_words_per_day > tuner {
                    continue;
                } else {
                    datasets.push(chapter_numbers.clone());
                    current_group_total_words = 0.0;
                    chapter_numbers.clear();
                }
            }

            // If any remaining chapters are not added, add them to the last dataset
            if !chapter_numbers.is_empty() {
                datasets.push(chapter_numbers.clone());
            }

            if (datasets.len() as f64) == n {
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
            } else if (datasets.len() as f64) < n {
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
fn adjust_dates(tcds: Vec<ChaptersDate>, bible_data: Vec<ChapterData>, end: NaiveDate) -> Vec<ChaptersDate> {
    let mut new_tcds: Vec<ChaptersDate> = tcds.clone();

    // find initial number of leftover days
    let last_date: NaiveDate = if let Some(ChaptersDate { date, .. }) = tcds.last() {
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

    // Add catch-up days at the end of the reading if applicable
    if num_days > 0 {
        let i = new_tcds.len() - 1;
        // Insert a new element
        let new_date = new_tcds[i].date + Duration::days(1);
        let new_element = ChaptersDate { titles: vec!["Catch-up day".to_string()], chapters: 0, date: new_date };
        new_tcds.push(new_element);

        num_days -= 1;
    }

    // Continue applying adjust_for_multiple_titles until num_days is no longer greater than 1
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
        let mut n = 1;

        for i in 0..new_tcds.len() - 1 {
            let current_titles = &new_tcds[i].titles;
            let next_titles = &new_tcds[i + 1].titles;
    
            if i > days_between * n && current_titles != next_titles {
                insert_new_element(&mut new_tcds, i, "Catch-up day".to_string(), 0);
                 n += 1;
            }
        }
    }

    new_tcds
}

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

fn write_to_file(filename: &str, adjusted_plan: Vec<ChaptersDate>) -> std::io::Result<()> {
    let mut file_path = PathBuf::from(env::current_dir()?);
    file_path.push(filename);
    let mut file = File::create(file_path)?;

    let mut old_title = String::new();
    let mut last_chapter: i32 = 0;
    let mut catch_up_num: u16 = 1;

    for t in adjusted_plan {
        let titles = t.titles.join(", ");
        let chapters = if t.titles.len() > 1 {
            "all".to_string()
        } else if titles == "Catch-up day" {
            format!("{}", catch_up_num)
        } else {
            if titles == old_title {
                if last_chapter == t.chapters || last_chapter == t.chapters - 1 {
                    format!("{}", t.chapters)
                } else {
                    format!("{}-{}", last_chapter + 1, t.chapters)
                }
            } else {
                if t.chapters == 1 {
                    format!("{}", t.chapters)
                } else {
                    format!("1-{}", t.chapters)
                }
            }
        };
        writeln!(file, "{} {} {}", t.date.format("%b %e, %Y"), titles, chapters)?;
        // println!("{} {} {}", t.date.format("%b %e, %Y"), titles, chapters);

        old_title = titles.clone();
        last_chapter = t.chapters;

        if titles == "Catch-up day" {
            catch_up_num += 1;
        }
    }
    Ok(())
}
