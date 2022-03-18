use std::collections::HashMap;
use std::sync::Arc;

use crate::backends::memory::MemoryBackend;
use crate::backends::cpu::CpuBackend;

use crate::backends::metric::Metric;
use crate::backends::metric::MetricValues;
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

// HashMap("metric_name", (id, "backend_name")))
// used to send shorter messages on the network, metric names are replaced by an id, this list must be the same in colmet-collector
// also used when adding metrics to get to find which backend can handle the requested metric
lazy_static! {  
    static ref METRIC_NAMES_MAP: HashMap<String, (i32, String)> = {
        let mut m = HashMap::new();
        let doc = load_metrics_from_yaml();
        let mut i = 0;
        for back in doc["metrics_order"].as_hash().unwrap() {
            let (b, array)=back;
            for metric in array.as_vec().unwrap() { 
                m.insert(metric.as_str().unwrap().to_string(), (i, b.as_str().unwrap().to_string()));
                i += 1;
            }
        }
        m
    };
    static ref METRICS_VERSION: i64 = load_metrics_from_yaml()["meta"]["version"].as_i64().unwrap() ;
    static ref NB_METRICS: usize = METRIC_NAMES_MAP.len();

}

// replace metric names by their id
pub fn compress_metric_names(metric_names: Vec<String>) -> Vec<i32> {
    println!("compress_metric_names");
    let mut res: Vec<i32> = Vec::new();
    for metric_name in metric_names {
        // println!("compress_metric_names metric_name {:#?}", metric_name.as_str().clone());
        res.push(METRIC_NAMES_MAP.get(metric_name.as_str()).unwrap().0);
    }
    res
}

pub trait Backend {
    fn say_hello(&self); // for debug
    fn get_backend_name(&self) -> String;
    fn open(&self);
    fn close(&self);
    fn get_metrics(& self) -> HashMap<i32, MetricValues>;
    fn get_some_metrics(&self, metrics_to_get: Vec<String>) -> HashMap<i32, MetricValues>;
    fn set_metrics_to_get(& self, metrics_to_get: Vec<String>);
}

pub struct BackendsManager {
    backends: Rc<RefCell<Vec<Box<dyn Backend>>>>,
    pub metrics_to_get: Vec<Metric>,
    pub last_timestamp: i64,
    pub last_measurement: HashMap<i32, (String, i64, i64, Vec<(String, Vec<i32>, Vec<i64>)>)>,
    pub metric_modified: bool,
}

impl BackendsManager {
    pub fn new(metrics: Vec<Metric>) -> BackendsManager {
        let backends = Rc::new(RefCell::new(Vec::new()));
        let mut metrics_to_get:Vec<Metric>=Vec::new();
        for m in metrics.clone() {
            let mut met=m.clone();
            met.backend_name=METRIC_NAMES_MAP.get(&m.metric_name).unwrap().1.clone();
            metrics_to_get.push(met);
        }
        //println!("{:?}", metrics_to_get);
        let last_measurement : HashMap<i32, (String, i64, i64, Vec<(String, Vec<i32>, Vec<i64>)>)>=HashMap::new();
        let last_timestamp=0 as i64;
        let metric_modified=false;
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
            //let perfhw_backend = PerfhwBackend::new(cgroup_manager.clone(), cli_args.metrics_to_get);
            //self.add_backend(Box::new(perfhw_backend));
        }

        return self.backends.clone();
    }

    pub fn add_backend(& self, backend: Box<dyn Backend>) {
        (*self.backends).borrow_mut().push(backend);
    }


// returns a HashMap
// job_id -> (hostname, timestamp, version, vec of(backend name, metric_names, metric_values))
    pub fn make_measure(& self, timestamp: i64, hostname: String) {
        /*let version = *METRICS_VERSION;
        if self.metric_modified {
            //self.last_measurement = HashMap::new();
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
        }*/
    }
    pub fn sort_waiting_metrics(& self){
        //self.metrics_to_get.sort_by_key(| k | k.0);
    }
    pub fn update_waiting_metrics(&self, delta_t: i64) -> Vec<String>{
        let list_metrics:Vec<String>=Vec::new();
       /*for i in 0..self.metrics_to_get.len() {
           if self.metrics_to_get[i].0 - delta_t < 0 {
               list_metrics.push(self.metrics_to_get[1].2);
               self.metrics_to_get[i].0=(self.metrics_to_get[i].1 * 1000.) as i64;
           }else{
               self.metrics_to_get[i].0-=delta_t;
           }
       }*/
       list_metrics
    }
    pub fn update_measurement(&self, job_id: i32, metrics: Vec<(String, Vec<i32>, Vec<i64>)>){
        let (_hostname, _timestamp, _version, measures)=self.last_measurement.get(&job_id).unwrap();
        let i=0;
        for measure in measures {
              if measure.0==metrics[i].0 && measure.1==metrics[i].1 {
                  //measure.2=metrics[i].2;
              }
        }
        //m.push((metric.backend_name, compress_metric_names(metric.metric_names), metric.metric_values.unwrap()));
    }
}
