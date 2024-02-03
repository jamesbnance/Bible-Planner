use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::error::Error;
use chrono::{ Duration, NaiveDate, Utc };
use csv::ReaderBuilder;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
struct Data {
    pub index: i32,
    pub title: String,
    pub chapters: i32,
    pub words: i32
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
    let start_date = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
    let end_date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap();
    assert!(end_date > start_date, "Invalid dates!");
    let duration: i32 = get_duration(start_date, end_date);

    let filename = format!("/home/jim/Documents/reading/plan_{}", Utc::now().timestamp());
    let path = Path::new(&filename);

    let bible_data: Vec<Data> = get_data("book_chapter.csv", book_index)?;

    let total_word_count: i32 = bible_data.iter().map(|b| b.words).sum();
    let avg_daily_word_count = total_word_count / duration;

    // Determine a vector of the books to read and the number of days for each
    let titles_chapters_days: Vec<ChaptersDays> = get_books_in_days(bible_data, duration);

    // Assign books and chapters to dates
    let titles_chapters_date: Vec<ChaptersDate> = get_chapters_dates(titles_chapters_days, start_date, end_date);

    // Adjust dates for Psalm 119 and to fill in catch-up days
    let final_vec: Vec<ChaptersDate> = adjust_dates(titles_chapters_date, end_date, avg_daily_word_count);

    // Write final plan to file
    if path.exists() {
        println!("File exists");
    } else {
        match write_to_file(&filename, final_vec) {
            Ok(_) => println!("Successfully wrote to file {}", &filename),
            Err(e) => {
                eprintln!("Failed to write to file: {}", e);
                std::process::exit(1);
            },
        }
    }
    Ok(())
}

// Create a vector with title, number of chapters, avg words per chapter
fn get_data(file_path: &str, book_index: Vec<i32>) -> Result<Vec<Data>, Box<dyn Error>> {
    let bible_file = File::open(file_path).expect("Unable to open file");
    let mut reader = ReaderBuilder::new().from_reader(bible_file);
    let mut data = Vec::new();
    for result in reader.deserialize() {
        let record: Data = result?;
        if !book_index.contains(&record.index) {
            continue;
        }
        data.push(record);
    }
    Ok(data)
}

// Find duration in days
fn get_duration(start: NaiveDate, end: NaiveDate) -> i32 {
    let duration_in_hms = end.and_hms_opt(0, 0, 0).unwrap() 
        - start.and_hms_opt(0, 0, 0).unwrap();
    duration_in_hms.num_days() as i32
}

// Determine a vector of the books to read and the number of days for each
fn get_books_in_days(bible_data: Vec<Data>, duration: i32) -> Vec<ChaptersDays> {
    let mut result = Vec::new();
    let mut temp_titles: Vec<String> = Vec::new();
    let mut temp_chapters: i32 = 0;
    let mut temp_days: f32 = 0.0;

    let total_word_count: i32 = bible_data.iter().map(|b| b.words).sum();

    for book in bible_data {
        let days: f32 = (book.words as f32 / total_word_count as f32) * duration as f32;
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

// Assign books and chapters to dates
fn get_chapters_dates(tcds: Vec<ChaptersDays>, start: NaiveDate, end: NaiveDate) -> Vec<ChaptersDate> {
    let mut title_chapters_dates = Vec::new();
    let mut current_date = start;

    for tcd in tcds {
        let chapters_per_day = tcd.chapters / tcd.days;
        let mut extra_chapters = tcd.chapters % tcd.days;
        let mut num_chapters: i32;
        let mut book_chapter = 0;
    
        for _day in 0..tcd.days {
            // Add one extra chapter each day until extra_chapters is zero
            if extra_chapters > 0 {
                num_chapters = chapters_per_day + 1;
                extra_chapters -= 1;
            } else {
                num_chapters = chapters_per_day;
            }

            book_chapter += num_chapters;
            
            let entry = ChaptersDate {
                titles: tcd.titles.clone(),
                chapters: book_chapter,
                date: current_date,
            };
            
            title_chapters_dates.push(entry);
            current_date = current_date.succ_opt().unwrap();

            if current_date > end {
                break;
            }            
        }
    }
    title_chapters_dates
}

// Adjust dates for Psalm 119, fill in catch-up days, split up combined readings if reasonable
fn adjust_dates(tcds: Vec<ChaptersDate>, end: NaiveDate, avg_daily_word_count: i32) -> Vec<ChaptersDate> {

    let mut new_tcds: Vec<ChaptersDate> = tcds.clone();

    // find initial number of leftover days
    let last_date: NaiveDate = if let Some(ChaptersDate { date, .. }) = tcds.last() {
        *date
    } else { end };
    let diff = end - last_date;
    let mut num_days = diff.num_days();

    // Adjust for Psalm 119, which has approx 2500 words
    if num_days > 0 && avg_daily_word_count < 2500 {
        for i in 0..new_tcds.len() - 1 {
            let current_chapters = new_tcds[i].chapters;
            let next_chapters = new_tcds[i + 1].chapters;
    
            if new_tcds[i].titles.contains(&"Psalms".to_string())
                && current_chapters < 119 && next_chapters >= 119 {
                insert_new_element(&mut new_tcds, i, "Psalms".to_string(), 119);
            }
        }
        num_days -= 1;
    }

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
                let base_chapters = max_chapters_element.chapters / num_titles;

                // Remove the original element
                new_tcds.remove(index);
    
                // Insert new elements for each title with adjusted dates
                for (i, title) in titles.iter().enumerate() {
                    let new_date = date + Duration::days(i as i64);
                    let new_element = ChaptersDate {
                        titles: vec![title.clone()],
                        chapters: base_chapters,
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

fn write_to_file(filename: &str, final_vec: Vec<ChaptersDate>) -> std::io::Result<()> {
    let mut file = File::create(filename)?;

    writeln!(file, "Date         Read through")?;
    for t in final_vec {
        let titles = t.titles.join(", ");
        let chapters = if t.titles.len() > 1 {
            "all".to_string()
        } else if titles == "Catch-up day" {
            "".to_string()
        } else {
            t.chapters.to_string()
        };
        writeln!(file, "{} {} {}", t.date.format("%b %e, %Y"), titles, chapters)?;
    }
    Ok(())
}
