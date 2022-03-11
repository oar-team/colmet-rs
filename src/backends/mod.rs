use std::collections::HashMap;
use std::sync::{Arc};

use crate::backends::memory::MemoryBackend;
use crate::backends::cpu::CpuBackend;

use crate::backends::metric::Metric;
use crate::cgroup_manager::CgroupManager;
use crate::CliArgs;

use crate::backends::perfhw::PerfhwBackend;

extern crate yaml_rust;
use yaml_rust::YamlLoader;

pub(crate) mod metric;

mod memory;
mod cpu;
mod perfhw;

use std::cell::RefCell;
use std::rc::Rc;


// start backends and periodically fetch all of them to get the metrics


fn load_metrics_from_yaml() -> yaml_rust::Yaml {
    YamlLoader::load_from_str(include_str!("metrics_order.yml"))
        .expect("failed to load YAML file")
        .pop()
        .unwrap()
}

// used to send more little messages on the network, metric names are replaced by an id, this list must be the same in colmet-collector
lazy_static! { 
    static ref METRIC_NAMES_MAP: HashMap< String, i32> = {
        let mut m = HashMap::new();
        let doc = load_metrics_from_yaml();
        let mut i = 0;
        for metric in doc["metrics_order"].as_vec().unwrap() {
            m.insert(metric.as_str().unwrap().to_string(), i);
            // println!("{:?}: {}", metric.as_str().unwrap(), i);
            i += 1;
        }
        m
    };
    static ref METRICS_VERSION: i64 = load_metrics_from_yaml()["meta"]["version"].as_i64().unwrap() ;
    static ref NB_METRICS: usize = METRIC_NAMES_MAP.len();

}

/* TOREMOVE
lazy_static! {
    static ref METRIC_NAMES_MAP: HashMap<&'static str, i32> = vec![
        ("cache", 1), // Memory Backend
        ("rss", 2),
        ("rss_huge", 3),
        ("shmem", 4),
        ("mapped_file", 5),
        ("dirty", 6),
        ("writeback", 7),
        ("swap", 65),
        ("pgpgin", 8),
        ("pgpgout", 9),
        ("pgfault", 10),
        ("pgmajfault", 11),
        ("inactive_anon", 12),
        ("active_anon", 13),
        ("inactive_file", 14),
        ("active_file", 15),
        ("unevictable", 16),
        ("hierarchical_memory_limit", 17),
        ("hierarchical_memsw_limit", 66),
        ("total_cache", 18),
        ("total_rss", 19),
        ("total_rss_huge", 20),
        ("total_shmem", 21),
        ("total_mapped_file", 22),
        ("total_dirty", 23),
        ("total_writeback", 24),
        ("total_swap", 67),
        ("total_pgpgin", 25),
        ("total_pgpgout", 26),
        ("total_pgfault", 27),
        ("total_pgmajfault", 28),
        ("total_inactive_anon", 29),
        ("total_active_anon", 30),
        ("total_inactive_file", 31),
        ("total_active_file", 32),
        ("total_unevictable", 33),
        ("nr_periods", 34), // Cpu Backend
        ("nr_throttled", 35),
        ("throttled_time", 36),
        ("cpu_cycles", 37), // Perfhw Backend
        ("instructions", 38),
        ("cache_references", 39),
        ("cache_misses", 40),
        ("branch_instructions", 41),
        ("branch_misses", 42),
        ("bus_cycles", 43),
        ("ref_cpu_cycles", 44),
        ("cache_l1d", 45),
        ("cache_ll", 46),
        ("cache_dtlb", 47),
        ("cache_itlb", 48),
        ("cache_bpu", 49),
        ("cache_node", 50),
        ("cache_op_read", 51),
        ("cache_op_prefetch", 52),
        ("cache_result_access", 53),
        ("cpu_clock", 54),
        ("task_clock", 55),
        ("page_faults", 56),
        ("context_switches", 57),
        ("cpu_migrations", 58),
        ("page_faults_min", 59),
        ("page_faults_maj", 60),
        ("alignment_faults", 61),
        ("emulation_faults", 62),
        ("dummy", 63),
        ("bpf_output", 64),
    ].into_iter().collect();
}
 */

// replace metric names by their id
pub fn compress_metric_names(metric_names: Vec<String>) -> Vec<i32> {
    println!("compress_metric_names");
    let mut res: Vec<i32> = Vec::new();
    for metric_name in metric_names {
        // println!("compress_metric_names metric_nzme {:#?}", metric_name.as_str().clone());
        res.push(*METRIC_NAMES_MAP.get(metric_name.as_str()).unwrap());
    }
    res
}

pub trait Backend {
    fn say_hello(&self); // for debug
    fn get_backend_name(&self) -> String;
    fn open(&self);
    fn close(&self);
    fn get_metrics(& self) -> HashMap<i32, Metric>;
    fn get_some_metrics(&self, metrics_to_get: Vec<String>) -> HashMap<i32, Metric>;
    fn set_metrics_to_get(& self, metrics_to_get: Vec<String>);
}

pub struct BackendsManager {
    backends: Rc<RefCell<Vec<Box<dyn Backend>>>>,
    pub metrics_to_get: Vec<(i64, f32, String)>,
    pub last_timestamp: i64,
    pub last_measurement: HashMap<i32, (String, i64, i64, Vec<(String, Vec<i32>, Vec<i64>)>)>,
    pub metric_modified: bool,
}

impl BackendsManager {
    pub fn new(metrics: Vec<(f32, String)>) -> BackendsManager {
        let backends = Rc::new(RefCell::new(Vec::new()));
        let metrics_to_get:Vec<(i64, f32, String)>=Vec::new();
        for m in metrics {
            metrics_to_get.push(((m.0*1000.)as i64, m.0, m.1));
        }
        let mut last_measurement : HashMap<i32, (String, i64, i64, Vec<(String, Vec<i32>, Vec<i64>)>)>=HashMap::new();
        let mut last_timestamp=0 as i64;
        let mut metric_modified=false;
        BackendsManager { backends, metrics_to_get, last_timestamp, last_measurement, metric_modified }
    }

    pub fn init_backends(& self, cli_args: CliArgs, cgroup_manager : Arc<CgroupManager>) ->  Rc<RefCell<Vec<Box<dyn Backend>>>> {
        let memory_backend = MemoryBackend::new(cgroup_manager.clone());
        let cpu_backend = CpuBackend::new(cgroup_manager.clone());
        self.add_backend(Box::new(memory_backend));
        self.add_backend(Box::new(cpu_backend));


        if cli_args.enable_infiniband {
            ()
        }
        if cli_args.enable_lustre {
            ()
        }
        if cli_args.enable_rapl {
            ()
        }
        if cli_args.enable_perfhw {
            let perfhw_backend = PerfhwBackend::new(cgroup_manager.clone(), cli_args.metrics_to_get);
            self.add_backend(Box::new(perfhw_backend));
        }

        return self.backends.clone();
    }

    pub fn add_backend(& self, backend: Box<dyn Backend>) {
        (*self.backends).borrow_mut().push(backend);
    }


// returns a HashMap
// job_id -> (hostname, timestamp, version, vec of(backend name, metric_names, metric_values))
    pub fn make_measure(& self, timestamp: i64, hostname: String) {
        let version = *METRICS_VERSION;
        if self.metric_modified {
            self.last_measurement = HashMap::new();
        }
        //let mut metrics: HashMap<i32, (String, i64, i64, Vec<(String, Vec<i32>, Vec<i64>)>)>= HashMap::new();
        
        let delta_t=self.last_timestamp-timestamp;
        let list_metrics=self.update_waiting_metrics(delta_t);

        let b = (*self.backends).borrow();
        let bi = b.iter();
        for backend in bi {
            for (job_id, metric) in backend.get_some_metrics(list_metrics) {
                match self.last_measurement.get_mut(&job_id) {
                    // if some metrics have already been added for the same job_id
                    Some(tmp) => {
                        let (_hostname, _timestamp, _version, m) = tmp;
                        self.update_measurement(job_id, m.clone());
                    },
                    // if no metrics were added for the job_id
                    None => {
                        self.last_measurement.insert(job_id, (hostname.clone(), timestamp, version, vec![(metric.backend_name, compress_metric_names(metric.metric_names), metric.metric_values.unwrap() )]));
                    },
                }
            }
        }
    }
    pub fn sort_waiting_metrics(& self){
        self.metrics_to_get.sort_by_key(| k | k.0);
    }
    pub fn update_waiting_metrics(&self, delta_t: i64) -> Vec<String>{
        let list_metrics:Vec<String>=Vec::new();
       for i in 0..self.metrics_to_get.len() {
           if self.metrics_to_get[i].0 - delta_t < 0 {
               list_metrics.push(self.metrics_to_get[1].2);
               self.metrics_to_get[i].0=(self.metrics_to_get[i].1 * 1000.) as i64;
           }else{
               self.metrics_to_get[i].0-=delta_t;
           }
       }
       list_metrics
    }
    pub fn update_measurement(&self, job_id: i32, metrics: Vec<(String, Vec<i32>, Vec<i64>)>){
        let (_hostname, _timestamp, _version, measures)=self.last_measurement.get_mut(&job_id).unwrap();
        let i=0;
        for measure in measures {
              if measure.0==metrics[i].0 && measure.1==metrics[i].1 {
                  measure.2=metrics[i].2;
              }
        }
        //m.push((metric.backend_name, compress_metric_names(metric.metric_names), metric.metric_values.unwrap()));
    }
}
