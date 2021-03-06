#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate regex;
extern crate serde_derive;
extern crate simple_logger;

use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, SystemTime};
use std::rc::Rc;
use std::cell::RefCell;
use std::path::Path;
use std::path::PathBuf;

use inotify::{
    EventMask,
    Inotify,
    WatchMask,
};

// command line argument parser
use clap::App;

use log::Level;

use crate::backends::BackendsManager;
use crate::cgroup_manager::CgroupManager;

mod backends;
mod cgroup_manager;
mod zeromq;


fn main(){

    let cli_args = parse_cli_args();

    let sample_period = Arc::new(Mutex::new(cli_args.sample_period));

    init_logger(cli_args.verbose);

    let cgroup_cpuset_path = format!("{}/cpuset{}", cli_args.cgroup_root_path.clone(),
                                      cli_args.cgroup_path_suffix.clone());
    
    if !Path::new(&cgroup_cpuset_path).exists() {
        debug!("cgroup_cpuset_path does not exist {}", cgroup_cpuset_path);
        if !cli_args.wait_cgroup_cpuset_path {
            error!("cgroup_cpuset_path does not exist {}", cgroup_cpuset_path);
            std::process::exit(1);
        } else {
            let mut inotify = Inotify::init()
                .expect("Failed to initialize inotify");
            let split_path = cgroup_cpuset_path.split("/");
            let mut path = PathBuf::from("/");
            let mut flag = false;

            debug!("Waiting cgroup_cpuset_path's creation {}: ", cgroup_cpuset_path);
            // Can be a recursive process
            for next in split_path {
                let dir = path.clone();    
                path.push(next);
                
                if !Path::new(&path).exists() {
                    inotify.add_watch(dir, WatchMask::CREATE,)
                        .expect("Failed to add inotify watch");
                    let mut buffer = [0u8; 4096];
                    while !Path::new(&path).exists() {
                        let events = inotify
                            .read_events_blocking(&mut buffer)
                            .expect("Failed to read inotify events");
                        for event in events {
                            if event.mask.contains(EventMask::ISDIR) {
                                debug!("Directory created: {:?}", event.name);
                                if Path::new(&cgroup_cpuset_path).exists() {
                                    flag = true;
                                }
                            }
                        }
                    }
                }
                if flag { break; }
            }
            inotify.close().expect("Failed to close inotify instance");
        }
    }

    // TODO, replace cgroup_root_path cgroup_path_suffix by cgroup_cpuset_path ?
    let cgroup_manager = CgroupManager::new(cli_args.regex_job_id.clone(),
                                            cli_args.cgroup_root_path.clone(),
                                            cli_args.cgroup_path_suffix.clone(),
                                            sample_period.clone(), cli_args.sample_period);

    let backends_manager_ref = Rc::new(RefCell::new(BackendsManager::new()));

    let bm = (*backends_manager_ref).borrow();
    let backends = bm.init_backends(cli_args.clone(), cgroup_manager.clone());

    let b_backends = &(*backends).borrow();
    let zmq_sender = zeromq::ZmqSender::init(b_backends);
    zmq_sender.open(&cli_args.zeromq_uri, cli_args.zeromq_linger, cli_args.zeromq_hwm);

    let hostname: String = gethostname::gethostname().to_str().unwrap().to_string();
    
    // main loop that pull backends measurements periodically ans send them with zeromq
    loop {
        let now = SystemTime::now();
        let timestamp = now.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as i64;
        println!("{:#?}", timestamp);

        let metric = bm.get_all_metrics(timestamp, hostname.clone());
        debug!("time to take measures {} microseconds", now.elapsed().unwrap().as_micros());
        zmq_sender.send_metrics(metric);
        zmq_sender.receive_config(sample_period.clone());
        sleep_to_round_timestamp((*(&*sample_period).lock().unwrap()  * 1000000000.0) as u128);

    }
}

/// sleep until the next timestamp that is a multiple of given duration_nanoseconds
/// as a consequence, the function sleeps a duration that is almost duration_nanoseconds and ends on a round timestamp
fn sleep_to_round_timestamp(duration_nanoseconds: u128) {
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
    let duration_to_sleep = ((now / duration_nanoseconds) + 1) * duration_nanoseconds - now;
    println!("sleeping for {:#?} milliseconds", duration_to_sleep/1000000);
    sleep(Duration::from_nanos(duration_to_sleep as u64));
}

#[derive(Clone)]
pub struct CliArgs {
    verbose: i32,
    sample_period: f64,
    enable_infiniband: bool,
    enable_lustre: bool,
    enable_perfhw: bool,
    enable_rapl: bool,
    zeromq_uri: String,
    zeromq_hwm: i32,
    zeromq_linger: i32,
    cgroup_root_path: String,
    cgroup_path_suffix: String,
    wait_cgroup_cpuset_path: bool,
    regex_job_id: String,
}

fn parse_cli_args() -> CliArgs {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();
    let verbose = matches.occurrences_of("verbose") as i32;
    let sample_period = value_t!(matches, "sample-period", f64).unwrap();
    println!("sample period {}", sample_period);
    let enable_infiniband = value_t!(matches, "enable-infiniband", bool).unwrap();
    let enable_lustre = value_t!(matches, "enable-lustre", bool).unwrap();
    let enable_perfhw = value_t!(matches, "enable-perfhw", bool).unwrap();
    let enable_rapl = value_t!(matches, "enable-RAPL", bool).unwrap();
    let zeromq_uri = value_t!(matches, "zeromq-uri", String).unwrap();
    let zeromq_hwm = value_t!(matches, "zeromq-hwm", i32).unwrap();
    let zeromq_linger = value_t!(matches, "zeromq-linger", i32).unwrap();
    let cgroup_root_path = value_t!(matches, "cgroup-root-path", String).unwrap();
    let cgroup_path_suffix = value_t!(matches, "cgroup-path-suffix", String).unwrap();
    let wait_cgroup_cpuset_path = value_t!(matches, "wait-cgroup-cpuset-path", bool).unwrap();
    let regex_job_id = value_t!(matches, "regex-job-id", String).unwrap();


    let cli_args = CliArgs {
        verbose,
        sample_period,
        enable_infiniband,
        enable_lustre,
        enable_perfhw,
        enable_rapl,
        zeromq_uri,
        zeromq_hwm,
        zeromq_linger,
        cgroup_root_path,
        cgroup_path_suffix,
        wait_cgroup_cpuset_path,
        regex_job_id
    };
    cli_args
}

fn init_logger(verbosity_lvl: i32) {
    match verbosity_lvl {
        0 => simple_logger::init_with_level(Level::Error).unwrap(),
        1 => simple_logger::init_with_level(Level::Warn).unwrap(),
        2 => simple_logger::init_with_level(Level::Info).unwrap(),
        3 => simple_logger::init_with_level(Level::Debug).unwrap(),
        _ => simple_logger::init_with_level(Level::Trace).unwrap(),
    }
}
