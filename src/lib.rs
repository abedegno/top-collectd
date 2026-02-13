use std::error;
use std::fs;
use std::io::{self, BufRead};
use std::thread;
use std::time::Duration;

use collectd_plugin::{
    collectd_plugin, ConfigItem, Plugin, PluginCapabilities, PluginManager, PluginRegistration,
    Value, ValueListBuilder,
};
use glob::glob;

#[derive(Default)]
struct TopCollectdPlugin;

#[derive(Debug)]
struct Proc {
    tid: u64,
    cputime: u64,
    cmd: String,
}

impl Proc {
    fn new(tid: u64, cputime: u64, cmd: &str) -> Proc {
        Proc {
            tid,
            cputime,
            cmd: cmd.to_string(),
        }
    }
}

#[derive(Debug)]
struct Top {
    cputime: u64,
    cmd: String,
    pcpu: f64,
}

impl Top {
    fn new(cputime: u64, cmd: &str, pcpu: f64) -> Top {
        Top {
            cputime,
            cmd: cmd.to_string(),
            pcpu,
        }
    }
}

/// Get the total CPU time from `/proc/stat`.
///
/// Sums the first 8 numeric fields (user, nice, system, idle, iowait, irq,
/// softirq, steal) from the aggregate `cpu` line.
fn get_total_cpu_time() -> Result<u64, Box<dyn error::Error>> {
    let file = fs::File::open("/proc/stat")?;
    let mut reader = io::BufReader::new(file);
    let mut line = String::new();
    reader.read_line(&mut line)?;

    let total: u64 = line
        .split_whitespace()
        .skip(1) // skip "cpu"
        .take(8) // user, nice, system, idle, iowait, irq, softirq, steal
        .filter_map(|s| s.parse::<u64>().ok())
        .sum();

    Ok(total)
}

/// Read process CPU times from `/proc/[pid]/stat`, sorted by tid.
///
/// Gracefully skips processes that disappear between the glob and read
/// (short-lived processes, permission errors, etc.).
fn get_processes_ordered() -> Vec<Proc> {
    let mut procs: Vec<Proc> = Vec::new();

    let entries = match glob("/proc/[0-9]*/stat") {
        Ok(entries) => entries,
        Err(_) => return procs,
    };

    for entry in entries.flatten() {
        let contents = match fs::read_to_string(&entry) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let fields: Vec<&str> = contents.split_whitespace().collect();
        if fields.len() < 15 {
            continue;
        }

        let tid = match fields[0].parse::<u64>() {
            Ok(t) => t,
            Err(_) => continue,
        };

        // utime (field 14) + stime (field 15) — 0-indexed as 13 and 14
        let utime = fields[13].parse::<u64>().unwrap_or(0);
        let stime = fields[14].parse::<u64>().unwrap_or(0);
        let cputime = utime + stime;

        // Strip parentheses from command name, e.g. "(httpd)" -> "httpd"
        let cmd = fields[1].trim_matches(|c| c == '(' || c == ')');

        procs.push(Proc::new(tid, cputime, cmd));
    }

    procs.sort_unstable_by_key(|p| p.tid);
    procs
}

// A manager decides the name of the family of plugins and also registers one
// or more plugins based on collectd's configuration files.
impl PluginManager for TopCollectdPlugin {
    fn name() -> &'static str {
        "top"
    }

    fn plugins(
        _config: Option<&[ConfigItem]>,
    ) -> Result<PluginRegistration, Box<dyn error::Error>> {
        Ok(PluginRegistration::Single(Box::new(TopCollectdPlugin)))
    }
}

impl Plugin for TopCollectdPlugin {
    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities::READ
    }

    fn read_values(&self) -> Result<(), Box<dyn error::Error>> {
        let num_cpus = num_cpus::get() as f64;

        // Take two snapshots 500ms apart to calculate CPU usage
        let total_cpu_time1 = get_total_cpu_time()?;
        let procs1 = get_processes_ordered();

        thread::sleep(Duration::from_millis(500));

        let total_cpu_time2 = get_total_cpu_time()?;
        let procs2 = get_processes_ordered();

        let total_cpu_time = total_cpu_time2.saturating_sub(total_cpu_time1);
        if total_cpu_time == 0 {
            return Ok(());
        }

        // Calculate per-process CPU usage using a merge-join on sorted tid lists
        let mut top_list: Vec<Top> = Vec::new();
        let (mut pos1, mut pos2) = (0, 0);

        while pos1 < procs1.len() && pos2 < procs2.len() {
            match procs1[pos1].tid.cmp(&procs2[pos2].tid) {
                std::cmp::Ordering::Less => pos1 += 1,
                std::cmp::Ordering::Greater => pos2 += 1,
                std::cmp::Ordering::Equal => {
                    let cputime = procs2[pos2].cputime.saturating_sub(procs1[pos1].cputime);
                    let pcpu = (cputime as f64 / total_cpu_time as f64) * 100.0 * num_cpus;

                    top_list.push(Top::new(cputime, &procs2[pos2].cmd, pcpu));
                    pos1 += 1;
                    pos2 += 1;
                }
            }
        }

        // Sort by CPU time descending and report top 10
        top_list.sort_by(|a, b| b.cputime.cmp(&a.cputime));

        for top in top_list.iter().take(10) {
            ValueListBuilder::new(Self::name(), "percent")
                .values(&[Value::Gauge(top.pcpu)])
                .plugin_instance(top.cmd.as_str())
                .type_instance("cpu")
                .submit()?;
        }

        Ok(())
    }
}

collectd_plugin!(TopCollectdPlugin);
