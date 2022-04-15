#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate regex;
extern crate serde_derive;
extern crate simple_logger;
//extern crate spin_sleep;

use std::thread::sleep;
use std::time::{Duration, SystemTime};
use std::fs;
use std::process::exit;
use std::collections::HashMap;
use std::rc::Rc;

use crate::utils::wait_file;
// command line argument parser
use clap::App;

use log::Level;

use crate::backends::BackendsManager;
use crate::cgroup_manager::CgroupManager;
use crate::backends::metric::Metric;

mod backends;
mod cgroup_manager;
mod utils;
mod zeromq;


fn main(){

    let mut measure_done:bool;

    let cli_args = parse_cli_args();

    init_logger(cli_args.verbose);
    
    if cli_args.verbose>=3 {
        debug!("{}", debug_list_metrics(cli_args.clone()));
    }
    let cgroup_cpuset_path = format!(
        "{}/cpuset{}",
        cli_args.cgroup_root_path.clone(),
        cli_args.cgroup_path_suffix.clone()
    );
    //let backends_manager_ref = Rc::new(RefCell::new(BackendsManager::new(cli_args.metrics_to_get.clone())));
    let mut backend_manager=BackendsManager::new(cli_args.sample_period, cli_args.metrics_to_get.clone());

    wait_file(&cgroup_cpuset_path, cli_args.wait_cgroup_cpuset_path);

    // TODO, replace cgroup_root_path cgroup_path_suffix by cgroup_cpuset_path ?
    let cgroup_manager = CgroupManager::new(cli_args.regex_job_id.clone(),
                                            cli_args.cgroup_root_path.clone(),
                                            cli_args.cgroup_path_suffix.clone(),);


    backend_manager.init_backends(cli_args.clone(), cgroup_manager.clone());
    let zmq_sender = zeromq::ZmqSender::init();
    zmq_sender.open(&cli_args.zeromq_uri, cli_args.zeromq_linger, cli_args.zeromq_hwm);

    let hostname: String = gethostname::gethostname().to_str().unwrap().to_string();
    
    // main loop that pull backends measurements periodically ans send them with zeromq
    loop {
        let config=zmq_sender.receive_config();
        if config.is_some(){
            let res:Rc<HashMap<String, String>>=Rc::new(config.unwrap());
            let sample_period:f32=res["sample_period"].clone().parse::<f32>().unwrap();
            match parse_metrics(res["metrics"].clone()){
                None => (),
                Some(new_metrics) => { backend_manager.update_metrics_to_get(sample_period, new_metrics); debug!("New metrics \\o/");  }
            }
        }
        let now = SystemTime::now();
        let timestamp = now.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as i64;
        println!("{:#?}", timestamp);

        //maybe compression needed here
        measure_done=backend_manager.make_measure(timestamp, hostname.clone());
        
        let time_to_take_measure=now.elapsed().unwrap().as_nanos();
        if measure_done {
            debug!("time to take measures {} microseconds", time_to_take_measure/1000);
            let m = backend_manager.last_measurement.clone();
            debug!("collected metrics : {:?}", m);
            zmq_sender.send_metrics(m);
        }else{
            debug!("Measure not done /o\\");
        }
        sleep_to_round_timestamp(backend_manager.get_sleep_time());
    }
}

/// sleep until the next timestamp that is a multiple of given duration_nanoseconds
/// as a consequence, the function sleeps a duration that is almost duration_nanoseconds and ends on a round timestamp
/// Round timestamp = millisecond granularity ?
/// to compensate for the ntp drift ? 
fn sleep_to_round_timestamp(duration_nanoseconds: u128) {
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
    let duration_to_sleep = ((now / duration_nanoseconds) + 1) * duration_nanoseconds - now;
    debug!("sleeping for {:#?} milliseconds", duration_to_sleep/1000000);
    sleep(Duration::from_nanos(duration_to_sleep as u64));
}

#[derive(Clone)]
pub struct CliArgs {
    verbose: i32,
    sample_period: f32,
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
    metrics_to_get: Vec<Metric>  
}

fn parse_cli_args() -> CliArgs {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();
    let verbose = value_t!(matches, "verbose", i32).unwrap();
    let sample_period = value_t!(matches, "sample-period", f32).unwrap();
    debug!("sample period {}", sample_period);
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

    let metrics_file = value_t!(matches, "file_metrics", String).unwrap();
    let mut metrics_to_get: Vec<Metric> = Vec::new();
    let mut arg_metrics:String;
    if !metrics_file.is_empty() { 
        if value_t!(matches, "metrics", String).unwrap().len()!=0 { 
        // specifying metrics and the file should fail
            println!("Do not specify metrics manually and a file");
            exit(0);
        }
        arg_metrics=fs::read_to_string(metrics_file)
            .expect("Specified metrics file doesn't exists");
        arg_metrics=arg_metrics.replace("\n", "");
    }
    else{
        arg_metrics = value_t!(matches, "metrics", String).unwrap();
    }
    if arg_metrics.is_empty() {
        metrics_to_get.push(Metric{job_id:-1, metric_name: "instructions".to_string(), backend_name: "perfhw".to_string(), sampling_period: -1., time_remaining_before_next_measure: (sample_period*1000.0) as i64});
        metrics_to_get.push(Metric{job_id:-1, metric_name: "pgfault".to_string(), backend_name: "Memory".to_string(), sampling_period: -1., time_remaining_before_next_measure: (sample_period*1000.0) as i64});
        metrics_to_get.push(Metric{job_id:-1, metric_name: "nr_periods".to_string(), backend_name: "Cpu".to_string(), sampling_period: -1., time_remaining_before_next_measure: (sample_period*1000.0) as i64});
    }else{
        match parse_metrics(arg_metrics){
            None => {
                println!("Could not parse provided metrics");
                //panic!("Could not parse provided metrics");
            }
            Some(m) => {
                metrics_to_get=m;
            }
        }
    }
    
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
        regex_job_id,
        metrics_to_get
    };
    cli_args
}

fn parse_metrics(arg_string: String) -> Option<Vec<Metric>> {
    let args=arg_string.split(",");
    let mut correct=true;
    let mut metrics = Vec::new();
    let mut s:f32;
    let mut n:String;
    let mut j:i32;
    for arg in args{
        let v:Vec<&str>=arg.split(":").collect();
        if v.len()==1 {
            n=v[0].to_string();
            s=-1.;
            j=-1;
        }else if v.len()==3 {
            n=v[0].to_string();
            s=v[1].to_string().parse::<f32>().unwrap();
            j=v[2].to_string().parse::<i32>().unwrap();
        }
        else{
            println!("Error while parsing metrics. Correct format is 'metric_name:sampling_period:job_id,...'. Sampling_period and job_id can be omited (they are set to -1).");
            correct=false;
            break;
        }
        let met = Metric {
            job_id: j,
            sampling_period: s,
            time_remaining_before_next_measure: (s*1000.0)as i64,
            metric_name:n,
            backend_name: "null".to_string(),
        };
        metrics.push(met);
    }
    if correct {
        return Some(metrics);
    }else{
        return None;
    }
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
fn debug_list_metrics(cli_args: CliArgs) -> String {
        let mut o:String ="".to_string();
        for s in &cli_args.metrics_to_get{
            if o.is_empty(){
                o=format!("{:?}", s);
            }
            else{
                o=format!("{}, {:?}", o, s);
            }
        }
        o=format!("List of metrics to collect\n\t{}", o);
        o
}
