use std::fs;

#[derive(Debug, PartialEq)]
pub enum CpuError {
    Io,
    NoData,
    MissingCpuLabel,
    MissingUser,
    InvalidUser,
    MissingNice,
    InvalidNice,
    MissingSystem,
    InvalidSystem,
    MissingIdle,
    InvalidIdle,
    MissingIowait,
    InvalidIowait,
}
pub struct CpuMonitor {
    previous: CpuTimes,
}
impl CpuMonitor {
    pub fn new() -> Result<Self, CpuError> {
        Ok(Self {
            previous: read_cpu_times()?,
        })
    }

    pub fn sample(&mut self) -> Result<f64, CpuError> {
        let current = read_cpu_times()?;

        let usage = self.previous.usage_between(&current);

        self.previous = current;

        Ok(usage)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuTimes {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
}
impl CpuTimes {
    fn total(&self) -> u64 {
        self.user + self.nice + self.system + self.idle + self.iowait
    }

    fn idle_time(&self) -> u64 {
        self.idle + self.iowait
    }

    fn usage_between(&self, newer: &CpuTimes) -> f64 {
        let total_difference = newer.total() - self.total();
        if total_difference == 0 {
            return 0.0;
        }

        let idle_difference = newer.idle_time() - self.idle_time();

        let busy_difference = total_difference - idle_difference;

        busy_difference as f64 / total_difference as f64 * 100.0
    }
}
fn parse_cpu_times(contents: &str) -> Result<CpuTimes, CpuError> {
    let first_line = contents.lines().next().ok_or(CpuError::NoData)?;

    let mut parts = first_line.split_whitespace();

    parts.next().ok_or(CpuError::MissingCpuLabel)?;

    let user: u64 = parts
        .next()
        .ok_or(CpuError::MissingUser)?
        .parse()
        .map_err(|_| CpuError::InvalidUser)?;

    let nice: u64 = parts
        .next()
        .ok_or(CpuError::MissingNice)?
        .parse()
        .map_err(|_| CpuError::InvalidNice)?;

    let system: u64 = parts
        .next()
        .ok_or(CpuError::MissingSystem)?
        .parse()
        .map_err(|_| CpuError::InvalidSystem)?;

    let idle: u64 = parts
        .next()
        .ok_or(CpuError::MissingIdle)?
        .parse()
        .map_err(|_| CpuError::InvalidIdle)?;

    let iowait: u64 = parts
        .next()
        .ok_or(CpuError::MissingIowait)?
        .parse()
        .map_err(|_| CpuError::InvalidIowait)?;

    Ok(CpuTimes {
        user,
        nice,
        system,
        idle,
        iowait,
    })
}
pub fn read_cpu_times() -> Result<CpuTimes, CpuError> {
    let contents = fs::read_to_string("/proc/stat").map_err(|_| CpuError::Io)?;

    parse_cpu_times(&contents)
} /*
pub fn read_cpu_usage() -> Result<f64, CpuError>{
let first = read_cpu_times()?;

thread::sleep(Duration::from_secs(1));

let second = read_cpu_times()?;

Ok(first.usage_between(&second))
}
*/
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_cpu_usage() {
        let first = CpuTimes {
            user: 100,
            nice: 0,
            system: 50,
            idle: 850,
            iowait: 0,
        };
        //100+0+50+850+0=1000
        let second = CpuTimes {
            user: 150,
            nice: 0,
            system: 100,
            idle: 1750,
            iowait: 0,
        };
        //150+0+100+1750+0=2000
        // The total difference is 2000 - 1000 = 1000
        //Idle difference is 1750 - 850 = 900
        //Busy difference is 1000 - 900 = 100
        //CPU usage is (100 / 1000) * 100 = 10.0%

        let usage = first.usage_between(&second);

        assert_eq!(usage, 10.0);
    }
    #[test]
    fn returns_zero_when_no_time_has_passed() {
        let first = CpuTimes {
            user: 100,
            nice: 0,
            system: 50,
            idle: 850,
            iowait: 0,
        };

        let second = CpuTimes {
            user: 100,
            nice: 0,
            system: 50,
            idle: 850,
            iowait: 0,
        };

        let usage = first.usage_between(&second);

        assert_eq!(usage, 0.0);
    }
    #[test]
    fn parses_cpu_times() {
        let data = "cpu 100 20 30 800 50";

        let cpu = parse_cpu_times(data).expect("Parsing should succeed");

        assert_eq!(cpu.user, 100);
        assert_eq!(cpu.nice, 20);
        assert_eq!(cpu.system, 30);
        assert_eq!(cpu.idle, 800);
        assert_eq!(cpu.iowait, 50);
    }
    #[test]

    fn fails_when_user_value_is_not_a_number() {
        let data = "cpu abc 20 30 800 50";

        let result = parse_cpu_times(data);
        assert_eq!(result, Err(CpuError::InvalidUser));
    }
    #[test]
    fn fails_when_idle_value_is_missing() {
        let data = "cpu 100 20 30";

        let result = parse_cpu_times(data);

        assert_eq!(result, Err(CpuError::MissingIdle));
    }
    #[test]
    fn fails_when_cpu_data_is_empty() {
        let data = "";

        let result = parse_cpu_times(data);

        assert_eq!(result, Err(CpuError::NoData));
    }

    #[test]
    fn fails_when_missing_user() {
        let data = "cpu";
        let result = parse_cpu_times(data);
        assert_eq!(result, Err(CpuError::MissingUser));
    }

    #[test]
    fn fails_when_missing_nice() {
        let data = "cpu 100";
        let result = parse_cpu_times(data);
        assert_eq!(result, Err(CpuError::MissingNice));
    }

    #[test]
    fn fails_when_invalid_nice() {
        let data = "cpu 100 abc 30 800 50";
        let result = parse_cpu_times(data);
        assert_eq!(result, Err(CpuError::InvalidNice));
    }

    #[test]
    fn fails_when_missing_system() {
        let data = "cpu 100 20";
        let result = parse_cpu_times(data);
        assert_eq!(result, Err(CpuError::MissingSystem));
    }

    #[test]
    fn fails_when_invalid_system() {
        let data = "cpu 100 20 abc 800 50";
        let result = parse_cpu_times(data);
        assert_eq!(result, Err(CpuError::InvalidSystem));
    }

    #[test]
    fn fails_when_invalid_idle() {
        let data = "cpu 100 20 30 abc 50";
        let result = parse_cpu_times(data);
        assert_eq!(result, Err(CpuError::InvalidIdle));
    }

    #[test]
    fn fails_when_missing_iowait() {
        let data = "cpu 100 20 30 800";
        let result = parse_cpu_times(data);
        assert_eq!(result, Err(CpuError::MissingIowait));
    }

    #[test]
    fn fails_when_invalid_iowait() {
        let data = "cpu 100 20 30 800 abc";
        let result = parse_cpu_times(data);
        assert_eq!(result, Err(CpuError::InvalidIowait));
    }

    #[test]
    fn parses_multiline_proc_stat_first_line() {
        let data =
            "cpu  200 40 60 1600 100 0 0 0 0 0\ncpu0 100 20 30 800 50\ncpu1 100 20 30 800 50";
        let cpu = parse_cpu_times(data).expect("Parsing should succeed");
        assert_eq!(cpu.user, 200);
        assert_eq!(cpu.nice, 40);
        assert_eq!(cpu.system, 60);
        assert_eq!(cpu.idle, 1600);
        assert_eq!(cpu.iowait, 100);
    }

    #[test]
    fn calculates_100_percent_cpu_usage() {
        let first = CpuTimes {
            user: 100,
            nice: 0,
            system: 50,
            idle: 850,
            iowait: 0,
        };
        let second = CpuTimes {
            user: 200,
            nice: 0,
            system: 150,
            idle: 850,
            iowait: 0,
        };
        // busy grew by (100 + 100) = 200, idle grew by 0, total grew by 200 -> 100%
        let usage = first.usage_between(&second);
        assert_eq!(usage, 100.0);
    }

    #[test]
    fn calculates_0_percent_when_all_time_is_idle() {
        let first = CpuTimes {
            user: 100,
            nice: 0,
            system: 50,
            idle: 850,
            iowait: 0,
        };
        let second = CpuTimes {
            user: 100,
            nice: 0,
            system: 50,
            idle: 1050,
            iowait: 0,
        };
        // total grew by 200, idle grew by 200, busy grew by 0 -> 0%
        let usage = first.usage_between(&second);
        assert_eq!(usage, 0.0);
    }

    #[test]
    fn calculates_total_and_idle_time() {
        let times = CpuTimes {
            user: 10,
            nice: 20,
            system: 30,
            idle: 40,
            iowait: 50,
        };
        assert_eq!(times.total(), 150);
        assert_eq!(times.idle_time(), 90);
    }
}
