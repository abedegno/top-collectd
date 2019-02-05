use std::io;
use std::io::BufRead;
use std::fs;
use std::fs::File;
use glob::glob;
use sysconf::raw::sysconf;
use sysconf::raw::SysconfVariable;
use std::time::Duration;
use shuteye::sleep;

extern crate shuteye;
extern crate glob;
extern crate sysconf;
#[macro_use]extern crate collectd_plugin;
#[macro_use]extern crate scan_fmt;

use collectd_plugin::{
    ConfigItem, Plugin, PluginCapabilities, PluginManager, PluginRegistration, Value,
    ValueListBuilder,
};
use std::error;

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
            tid: tid,
            cputime: cputime,
            cmd: cmd.to_string()
        }
    }
}

#[derive(Debug)]
struct Top {
    tid: u64,
    cputime: u64,    
    cmd: String,
    pcpu: f64
}

impl Top {
    fn new(tid: u64, cputime: u64, cmd: &str, pcpu: f64) -> Top {
        Top {
            tid: tid,
            cputime: cputime,            
            cmd: cmd.to_string(),
            pcpu: pcpu
        }
    }
}



// Get the total CPU time from /proc/stat
// TODO better error handling
fn get_total_cpu_time() -> u64 {
	let file = File::open("/proc/stat").expect("Can't open file");
	let mut reader = io::BufReader::new(file);
	let mut line = String::new();
    let _len = reader.read_line(&mut line).expect("Error reading line");

    // TODO More efficien way of capturing this?
    let (user, nice, system, idle, iowait, irq, softirq, steal, _guest, _guestnice) = scan_fmt!(&line.to_string(), // input string
		"cpu  {} {} {} {} {} {} {} {} {} {}",   // format
        u64, u64, u64, u64, u64, u64, u64, u64, u64, u64);

    return user.unwrap_or(0) 
    	+ nice.unwrap_or(0) 
    	+ system.unwrap_or(0) 
    	+ idle.unwrap_or(0) 
    	+ iowait.unwrap_or(0) 
    	+ irq.unwrap_or(0) 
    	+ softirq.unwrap_or(0) 
    	+ steal.unwrap_or(0);
}

fn get_processes_ordered() -> Vec<Proc> {
	// GET Processes
	let mut procs :Vec<Proc> = Vec::new();
	let res = glob("/proc/[0-9]*/stat").expect("Can't glob path");
	for entry in res
	{
		let path = entry.expect("No filename");
		let contents = fs::read_to_string(path)
			.expect("Something went wrong reading the file");
		
		let split: Vec<&str> = contents.split(" ").collect();

		// utime + stime
		let cputime = split.get(13).unwrap().parse::<u64>().unwrap() + split.get(14).unwrap().parse::<u64>().unwrap();

		let proc = Proc::new(
			split.get(0).unwrap().parse::<u64>().unwrap(),	// tid
			cputime,
			split.get(1).unwrap() // cmd
			);
		procs.push(proc);
	}

    // Sort by tid ascending
	procs.sort_unstable_by(|a, b| a.tid.cmp(&b.tid));
	return procs
}



// A manager decides the name of the family of plugins and also registers one or more plugins based
// on collectd's configuration files
impl PluginManager for TopCollectdPlugin {
    // A plugin needs a unique name to be referenced by collectd
    fn name() -> &'static str {
        "top"
    }

    // Our plugin might have configuration section in collectd.conf, which will be passed here if
    // present. Our contrived plugin doesn't care about configuration so it returns only a single
    // plugin (itself).
    fn plugins(_config: Option<&[ConfigItem]>) -> Result<PluginRegistration, Box<error::Error>> {
        Ok(PluginRegistration::Single(Box::new(TopCollectdPlugin)))
    }
}

impl Plugin for TopCollectdPlugin {
    // We define that our plugin will only be reporting / submitting values to writers
    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities::READ
    }

    fn read_values(&self) -> Result<(), Box<error::Error>> {
        // Create a list of values to submit to collectd. 

		// Get CPU count
		let num_cpus = sysconf(SysconfVariable::ScNprocessorsOnln).unwrap();	

		// Get initial total CPU Time
		let total_cpu_time1 = get_total_cpu_time();

		// Get initial process CPU usage
		let procs1 = get_processes_ordered();

	    /* 500 ms in ns */
	    sleep(Duration::new(0, 500 * 1000 * 1000));

	    // Get after total CPU time
	    let total_cpu_time2 = get_total_cpu_time();

	    // Get after process CPU usage
		let procs2 = get_processes_ordered();	

		// Calculate difference in total CPU time
	    let total_cpu_time = total_cpu_time2 - total_cpu_time1;

		// Calculate CPU usage between readings
		let mut top_list :Vec<Top> = Vec::new();
		let (mut pos1, mut pos2) = (0usize, 0usize);
		while pos1 < procs1.len() && pos2 < procs2.len()
		{
			if procs1[pos1].tid < procs2[pos2].tid
			{
				pos1+=1;
			}
			else if procs1[pos1].tid > procs2[pos2].tid {
				pos2+=1;
			}
			else 
			{
				let cputime = procs2[pos2].cputime - procs1[pos1].cputime;
				let top = Top::new(
					procs2[pos2].tid,	// tid
					cputime,					
					&procs2[pos2].cmd, // cmd
					(cputime  as f64 / total_cpu_time  as f64) * 100.0 * num_cpus as f64
				);
				;
				top_list.push(top);
				pos2+=1;
				pos1+=1;
			}
		}

		// Sort by CPU time descending
		top_list.sort_by(|a, b| b.cputime.cmp(&a.cputime));

		for i in (0..top_list.len()).take(10)
		{
			let top = &top_list[i];
			let values = vec![Value::Gauge(top.pcpu)];

	        ValueListBuilder::new(Self::name(), "percent")
	            .values(&values)
	            .plugin_instance(&top.cmd as &str)
	            .type_instance("cpu")
	            .submit()?;
		}
        

        // Submit our values to collectd. A plugin can submit any number of times.

        Ok(())
    }
}

// We pass in our plugin manager type
collectd_plugin!(TopCollectdPlugin);
