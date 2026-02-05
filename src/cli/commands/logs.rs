use crate::config::paths::Paths;
use std::fs;
use std::io::{BufRead, BufReader};
use std::time::{Duration, SystemTime};

pub async fn handle_logs(follow: bool, tail: u32, since: Option<String>) -> anyhow::Result<()> {
    let log_path = Paths::state_dir().join("daemon.log");

    if !log_path.exists() {
        println!("No log file found");
        return Ok(());
    }

    if follow {
        handle_follow(&log_path, tail).await?;
    } else if let Some(duration_str) = since {
        handle_since(&log_path, &duration_str)?;
    } else if tail > 0 {
        handle_tail(&log_path, tail)?;
    } else {
        handle_all(&log_path)?;
    }

    Ok(())
}

fn handle_all(log_path: &std::path::Path) -> anyhow::Result<()> {
    let content = fs::read_to_string(log_path)?;
    print!("{}", content);
    Ok(())
}

fn handle_tail(log_path: &std::path::Path, tail: u32) -> anyhow::Result<()> {
    let file = fs::File::open(log_path)?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;

    let start = if lines.len() > tail as usize {
        lines.len() - tail as usize
    } else {
        0
    };

    for line in &lines[start..] {
        println!("{}", line);
    }

    Ok(())
}

fn handle_since(log_path: &std::path::Path, duration_str: &str) -> anyhow::Result<()> {
    let duration = parse_duration(duration_str)?;
    let cutoff_time = SystemTime::now() - duration;

    let file = fs::File::open(log_path)?;
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = line?;
        if should_include_line(&line, cutoff_time) {
            println!("{}", line);
        }
    }

    Ok(())
}

async fn handle_follow(log_path: &std::path::Path, tail: u32) -> anyhow::Result<()> {
    let file = fs::File::open(log_path)?;
    let reader = BufReader::new(file);
    let lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;

    let start = if lines.len() > tail as usize {
        lines.len() - tail as usize
    } else {
        0
    };

    for line in &lines[start..] {
        println!("{}", line);
    }

    let mut last_size = fs::metadata(log_path)?.len();

    loop {
        tokio::time::sleep(Duration::from_millis(500)).await;

        if let Ok(metadata) = fs::metadata(log_path) {
            let current_size = metadata.len();
            if current_size > last_size {
                let file = fs::File::open(log_path)?;
                let reader = BufReader::new(file);
                for line in reader.lines() {
                    let line = line?;
                    println!("{}", line);
                }
                last_size = current_size;
            }
        }
    }
}

fn parse_duration(duration_str: &str) -> anyhow::Result<Duration> {
    let duration_str = duration_str.trim();
    let (num_str, unit) = if let Some(pos) = duration_str.find(|c: char| c.is_alphabetic()) {
        (&duration_str[..pos], &duration_str[pos..])
    } else {
        anyhow::bail!("Invalid duration format: {}", duration_str);
    };

    let num: u64 = num_str.parse()?;

    let duration = match unit {
        "s" | "sec" | "second" | "seconds" => Duration::from_secs(num),
        "m" | "min" | "minute" | "minutes" => Duration::from_secs(num * 60),
        "h" | "hour" | "hours" => Duration::from_secs(num * 3600),
        "d" | "day" | "days" => Duration::from_secs(num * 86400),
        _ => anyhow::bail!("Unknown duration unit: {}", unit),
    };

    Ok(duration)
}

fn should_include_line(line: &str, cutoff_time: SystemTime) -> bool {
    if let Some(timestamp_str) = extract_timestamp(line) {
        if let Ok(line_time) = parse_timestamp(timestamp_str) {
            return line_time >= cutoff_time;
        }
    }
    false
}

fn extract_timestamp(line: &str) -> Option<&str> {
    if line.len() >= 19 {
        let potential_ts = &line[..19];
        if potential_ts.chars().nth(4) == Some('-')
            && potential_ts.chars().nth(7) == Some('-')
            && potential_ts.chars().nth(10) == Some('T')
            && potential_ts.chars().nth(13) == Some(':')
            && potential_ts.chars().nth(16) == Some(':')
        {
            return Some(potential_ts);
        }
    }
    None
}

fn parse_timestamp(timestamp_str: &str) -> anyhow::Result<SystemTime> {
    let parts: Vec<&str> = timestamp_str.split(['-', 'T', ':']).collect();
    if parts.len() < 6 {
        anyhow::bail!("Invalid timestamp format");
    }

    let year: u32 = parts[0].parse()?;
    let month: u32 = parts[1].parse()?;
    let day: u32 = parts[2].parse()?;
    let hour: u32 = parts[3].parse()?;
    let minute: u32 = parts[4].parse()?;
    let second: u32 = parts[5].parse()?;

    let days_since_epoch = days_since_unix_epoch(year, month, day)?;
    let secs_since_epoch =
        days_since_epoch as u64 * 86400 + hour as u64 * 3600 + minute as u64 * 60 + second as u64;

    Ok(SystemTime::UNIX_EPOCH + Duration::from_secs(secs_since_epoch))
}

fn days_since_unix_epoch(year: u32, month: u32, day: u32) -> anyhow::Result<i64> {
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        anyhow::bail!("Invalid date");
    }

    let mut days: i64 = 0;

    for y in 1970..year {
        days += if is_leap_year(y) { 366 } else { 365 };
    }

    let days_in_month = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 1..month {
        let mut m_days = days_in_month[(m - 1) as usize];
        if m == 2 && is_leap_year(year) {
            m_days = 29;
        }
        days += m_days as i64;
    }

    days += (day - 1) as i64;

    Ok(days)
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}
