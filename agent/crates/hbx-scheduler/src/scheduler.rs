use std::time::Duration;

use chrono::{DateTime, Datelike, NaiveTime, TimeZone, Timelike, Utc, Weekday};

use hbx_core::domain::common::ScheduleId;
use hbx_core::domain::schedule::{Schedule, ScheduleMode};

pub struct Scheduler;

impl Scheduler {
    pub fn compute_next_run(schedule: &Schedule, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match schedule.mode {
            ScheduleMode::Manual => None,
            ScheduleMode::Interval => Self::next_interval(schedule, now),
            ScheduleMode::Daily => Self::next_daily(schedule, now),
            ScheduleMode::Weekly => Self::next_weekly(schedule, now),
            ScheduleMode::Monthly => Self::next_monthly(schedule, now),
            ScheduleMode::Cron => Self::next_cron(schedule, now),
        }
    }

    pub fn should_run_now(schedule: &Schedule, now: DateTime<Utc>) -> bool {
        match schedule.next_run_at {
            Some(next) => now >= next,
            None => false,
        }
    }

    pub fn handle_missed(schedule: &Schedule, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        if let Some(next) = schedule.next_run_at {
            if next < now {
                return Some(next);
            }
        }
        None
    }

    pub fn update_after_run(schedule: &mut Schedule, now: DateTime<Utc>) {
        schedule.next_run_at = Self::compute_next_run(schedule, now);
    }

    fn next_interval(schedule: &Schedule, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let interval_secs = schedule.interval?;
        Some(now + Duration::from_secs(interval_secs))
    }

    fn next_daily(schedule: &Schedule, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let time_str = schedule.time_of_day.as_ref()?;
        let time = parse_time_of_day(time_str)?;
        let today_target = now.date_naive().and_time(time);
        let today_target_utc = Utc.from_utc_datetime(&today_target);

        if today_target_utc > now {
            Some(today_target_utc)
        } else {
            Some(today_target_utc + Duration::from_secs(86400))
        }
    }

    fn next_weekly(schedule: &Schedule, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let target_dow = schedule.day_of_week?;
        let time_str = schedule.time_of_day.as_ref()?;
        let time = parse_time_of_day(time_str)?;

        let weekday = match target_dow {
            0 => Weekday::Sun,
            1 => Weekday::Mon,
            2 => Weekday::Tue,
            3 => Weekday::Wed,
            4 => Weekday::Thu,
            5 => Weekday::Fri,
            6 => Weekday::Sat,
            _ => return None,
        };

        let today = now.date_naive();
        let current_dow = today.weekday();
        let days_until = (weekday.num_days_from_monday() as i64
            - current_dow.num_days_from_monday() as i64
            + 7)
            % 7;

        let target_date = today + chrono::Duration::days(days_until);
        let target = target_date.and_time(time);
        let target_utc = Utc.from_utc_datetime(&target);

        if target_utc > now {
            Some(target_utc)
        } else {
            Some(target_utc + Duration::from_secs(7 * 86400))
        }
    }

    fn next_monthly(schedule: &Schedule, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let target_dom = schedule.day_of_month?;
        let time_str = schedule.time_of_day.as_ref()?;
        let time = parse_time_of_day(time_str)?;

        let today = now.date_naive();
        let year = today.year();
        let month = today.month();

        let target_this_month = chrono::NaiveDate::from_ymd_opt(year, month, target_dom as u32)?;
        let target = target_this_month.and_time(time);
        let target_utc = Utc.from_utc_datetime(&target);

        if target_utc > now {
            Some(target_utc)
        } else {
            let next_month = if month == 12 { 1 } else { month + 1 };
            let next_year = if month == 12 { year + 1 } else { year };
            let target_next = chrono::NaiveDate::from_ymd_opt(next_year, next_month, target_dom as u32)?;
            Some(Utc.from_utc_datetime(&target_next.and_time(time)))
        }
    }

    fn next_cron(schedule: &Schedule, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let expr = schedule.cron_expression.as_ref()?;
        let cron = CronExpr::parse(expr)?;
        cron.next_after(now)
    }
}

fn parse_time_of_day(s: &str) -> Option<NaiveTime> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let hour: u32 = parts[0].parse().ok()?;
    let minute: u32 = parts[1].parse().ok()?;
    NaiveTime::from_hms_opt(hour, minute, 0)
}

#[derive(Debug, Clone)]
struct CronExpr {
    minutes: Vec<u8>,
    hours: Vec<u8>,
    days_of_month: Vec<u8>,
    months: Vec<u8>,
    days_of_week: Vec<u8>,
}

impl CronExpr {
    fn parse(expr: &str) -> Option<Self> {
        let fields: Vec<&str> = expr.split_whitespace().collect();
        if fields.len() != 5 {
            return None;
        }

        Some(Self {
            minutes: parse_field(fields[0], 0, 59)?,
            hours: parse_field(fields[1], 0, 23)?,
            days_of_month: parse_field(fields[2], 1, 31)?,
            months: parse_field(fields[3], 1, 12)?,
            days_of_week: parse_field(fields[4], 0, 6)?,
        })
    }

    fn next_after(&self, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let mut candidate = after + Duration::from_secs(60);
        let limit = after + Duration::from_secs(366 * 86400);

        while candidate < limit {
            let min = candidate.minute() as u8;
            let hour = candidate.hour() as u8;
            let dom = candidate.day() as u8;
            let month = candidate.month() as u8;
            let dow = candidate.weekday().num_days_from_sunday() as u8;

            if self.minutes.contains(&min)
                && self.hours.contains(&hour)
                && self.days_of_month.contains(&dom)
                && self.months.contains(&month)
                && self.days_of_week.contains(&dow)
            {
                let naive = candidate.date_naive()
                    .and_hms_opt(candidate.hour(), candidate.minute(), 0)?;
                return Some(Utc.from_utc_datetime(&naive));
            }

            candidate += Duration::from_secs(60);
        }

        None
    }
}

fn parse_field(field: &str, min: u8, max: u8) -> Option<Vec<u8>> {
    if field == "*" {
        return Some((min..=max).collect());
    }

    if let Some(step_str) = field.strip_prefix("*/") {
        let step: u8 = step_str.parse().ok()?;
        if step == 0 {
            return None;
        }
        return Some((min..=max).step_by(step as usize).collect());
    }

    let mut result = Vec::new();
    for part in field.split(',') {
        if let Some((start_str, end_str)) = part.split_once('-') {
            let start: u8 = start_str.parse().ok()?;
            let end: u8 = end_str.parse().ok()?;
            for v in start..=end {
                if v >= min && v <= max {
                    result.push(v);
                }
            }
        } else {
            let v: u8 = part.parse().ok()?;
            if v >= min && v <= max {
                result.push(v);
            }
        }
    }

    if result.is_empty() {
        None
    } else {
        result.sort();
        result.dedup();
        Some(result)
    }
}

pub fn create_schedule(
    schedule_id: ScheduleId,
    mode: ScheduleMode,
) -> Schedule {
    Schedule {
        schedule_id,
        mode,
        cron_expression: None,
        interval: None,
        time_of_day: None,
        day_of_week: None,
        day_of_month: None,
        next_run_at: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_schedule(mode: ScheduleMode) -> Schedule {
        create_schedule(ScheduleId(Uuid::new_v4()), mode)
    }

    #[test]
    fn test_manual_mode_no_auto_trigger() {
        let schedule = make_schedule(ScheduleMode::Manual);
        let now = Utc::now();
        assert!(Scheduler::compute_next_run(&schedule, now).is_none());
    }

    #[test]
    fn test_interval_mode() {
        let mut schedule = make_schedule(ScheduleMode::Interval);
        schedule.interval = Some(3600);
        let now = Utc::now();
        let next = Scheduler::compute_next_run(&schedule, now).unwrap();
        assert!(next > now);
        let diff = (next - now).num_seconds();
        assert_eq!(diff, 3600);
    }

    #[test]
    fn test_daily_mode_future_today() {
        let mut schedule = make_schedule(ScheduleMode::Daily);
        schedule.time_of_day = Some("23:59".to_string());
        let now = Utc::now();
        let next = Scheduler::compute_next_run(&schedule, now).unwrap();
        assert!(next > now);
    }

    #[test]
    fn test_daily_mode_past_today() {
        let mut schedule = make_schedule(ScheduleMode::Daily);
        schedule.time_of_day = Some("00:01".to_string());
        let now = Utc::now() + Duration::from_secs(120);
        let next = Scheduler::compute_next_run(&schedule, now).unwrap();
        assert!(next > now);
    }

    #[test]
    fn test_weekly_mode() {
        let mut schedule = make_schedule(ScheduleMode::Weekly);
        schedule.day_of_week = Some(1);
        schedule.time_of_day = Some("03:00".to_string());
        let now = Utc::now();
        let next = Scheduler::compute_next_run(&schedule, now).unwrap();
        assert!(next >= now);
    }

    #[test]
    fn test_monthly_mode() {
        let mut schedule = make_schedule(ScheduleMode::Monthly);
        schedule.day_of_month = Some(15);
        schedule.time_of_day = Some("02:00".to_string());
        let now = Utc::now();
        let next = Scheduler::compute_next_run(&schedule, now);
        assert!(next.is_some());
    }

    #[test]
    fn test_cron_mode_every_minute() {
        let mut schedule = make_schedule(ScheduleMode::Cron);
        schedule.cron_expression = Some("* * * * *".to_string());
        let now = Utc::now();
        let next = Scheduler::compute_next_run(&schedule, now).unwrap();
        assert!(next > now);
        let diff = (next - now).num_seconds();
        assert!(diff <= 120);
    }

    #[test]
    fn test_cron_mode_specific_time() {
        let mut schedule = make_schedule(ScheduleMode::Cron);
        schedule.cron_expression = Some("0 3 * * *".to_string());
        let now = Utc::now();
        let next = Scheduler::compute_next_run(&schedule, now).unwrap();
        assert!(next > now);
        assert_eq!(next.hour(), 3);
        assert_eq!(next.minute(), 0);
    }

    #[test]
    fn test_cron_mode_step() {
        let mut schedule = make_schedule(ScheduleMode::Cron);
        schedule.cron_expression = Some("*/15 * * * *".to_string());
        let now = Utc::now();
        let next = Scheduler::compute_next_run(&schedule, now).unwrap();
        assert!(next.minute().is_multiple_of(15));
    }

    #[test]
    fn test_cron_mode_range() {
        let mut schedule = make_schedule(ScheduleMode::Cron);
        schedule.cron_expression = Some("0 9-17 * * 1-5".to_string());
        let now = Utc::now();
        let next = Scheduler::compute_next_run(&schedule, now).unwrap();
        assert!(next.hour() >= 9 && next.hour() <= 17);
    }

    #[test]
    fn test_should_run_now() {
        let mut schedule = make_schedule(ScheduleMode::Interval);
        schedule.interval = Some(60);
        let now = Utc::now();
        schedule.next_run_at = Some(now - Duration::from_secs(10));
        assert!(Scheduler::should_run_now(&schedule, now));

        schedule.next_run_at = Some(now + Duration::from_secs(10));
        assert!(!Scheduler::should_run_now(&schedule, now));
    }

    #[test]
    fn test_handle_missed() {
        let mut schedule = make_schedule(ScheduleMode::Interval);
        schedule.interval = Some(3600);
        let now = Utc::now();
        schedule.next_run_at = Some(now - Duration::from_secs(7200));
        let missed = Scheduler::handle_missed(&schedule, now);
        assert!(missed.is_some());

        schedule.next_run_at = Some(now + Duration::from_secs(3600));
        let not_missed = Scheduler::handle_missed(&schedule, now);
        assert!(not_missed.is_none());
    }

    #[test]
    fn test_update_after_run() {
        let mut schedule = make_schedule(ScheduleMode::Interval);
        schedule.interval = Some(3600);
        let now = Utc::now();
        Scheduler::update_after_run(&mut schedule, now);
        assert!(schedule.next_run_at.is_some());
        assert!(schedule.next_run_at.unwrap() > now);
    }

    #[test]
    fn test_cron_parse_invalid() {
        assert!(CronExpr::parse("invalid").is_none());
        assert!(CronExpr::parse("* * * *").is_none());
        assert!(CronExpr::parse("* * * * * *").is_none());
    }

    #[test]
    fn test_parse_time_of_day() {
        assert!(parse_time_of_day("12:30").is_some());
        assert!(parse_time_of_day("00:00").is_some());
        assert!(parse_time_of_day("23:59").is_some());
        assert!(parse_time_of_day("24:00").is_none());
        assert!(parse_time_of_day("12").is_none());
        assert!(parse_time_of_day("abc").is_none());
    }
}