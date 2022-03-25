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
    fn return_values(&self, metrics_to_get: HashMap<i32, Vec<Metric>>) -> HashMap<i32, MetricValues>;
}

pub struct BackendsManager {
    backends: Rc<RefCell<Vec<Box<dyn Backend>>>>,
    pub metrics_to_get: Vec<Metric>,
    pub last_timestamp: i64,
    pub sample_period: i64,
    pub last_measurement: HashMap<i32, (String, i64, i64, Vec<MetricValues>)>,
    pub metrics_modified: bool,
}

impl BackendsManager {
    pub fn new(sp: f64, metrics: Vec<Metric>) -> BackendsManager {
        let backends = Rc::new(RefCell::new(Vec::new()));
        let mut metrics_to_get:Vec<Metric>=Vec::new();
        for m in metrics.clone() {
            let mut met=m.clone();
            met.backend_name=METRIC_NAMES_MAP.get(&m.metric_name).unwrap().1.clone();
            metrics_to_get.push(met);
        }
        //println!("{:?}", metrics_to_get);
        let last_measurement : HashMap<i32, (String, i64, i64, Vec<MetricValues>)>=HashMap::new();
        let last_timestamp=0 as i64;
        let metrics_modified=false;
        let sample_period=(sp*1000.)as i64;
        BackendsManager { backends, metrics_to_get, last_timestamp, last_measurement, metrics_modified, sample_period }
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
            let perfhw_backend = PerfhwBackend::new(cgroup_manager.clone());
            self.add_backend(Box::new(perfhw_backend));
        }
        return self.backends.clone();
    }

    pub fn add_backend(& self, backend: Box<dyn Backend>) {
        (*self.backends).borrow_mut().push(backend);
    }


// returns a HashMap
// job_id -> (hostname, timestamp, version, vec of MetricValues)
    pub fn make_measure(&mut self, timestamp: i64, hostname: String) {
        let version = *METRICS_VERSION;
        if self.metrics_modified {
            self.last_measurement = HashMap::new();
        }
        if self.last_timestamp==0 {
            self.last_timestamp=timestamp;
        }
        for m in self.metrics_to_get.clone() {
            println!("{} {:?}", m.metric_name, m.time_remaining_before_next_measure);
        }
        let delta_t=timestamp-self.last_timestamp;
        self.last_timestamp=timestamp;
        let mut list_metrics=self.get_metrics_to_collect_now(delta_t);
        println!("list of metrics to get now (delta_t :{}) {:?}\n", delta_t, list_metrics);
        let b = (*self.backends).borrow();
        let bi = b.iter();
        for backend in bi {
            //println!("backend {}", backend.get_backend_name());
            if list_metrics.get_mut(&(backend.get_backend_name())).is_none(){
                continue;
            }
            //println!("metrics to get for backend {} :\n {:?}", backend.get_backend_name(),  list_metrics.get(&(backend.get_backend_name())).unwrap());
            for (job_id, metric) in backend.return_values(list_metrics.get_mut(&(backend.get_backend_name())).unwrap().clone()) {
                //println!("metric values : {} {:?}", job_id, metric);
                match self.last_measurement.get(&job_id) {
                    // if some metrics have already been added for the same job_id
                    Some(tmp) => {
                        let (_hostname, _timestamp, _version, m) = tmp;
                        self.update_measurement(job_id, m.clone());
                    },
                    // if no metrics were added for the job_id
                    None => {
                        let mut v:Vec<MetricValues>=Vec::new();
                        v.push(metric);
                        self.last_measurement.insert(job_id, (hostname.clone(), timestamp, version, v.clone()));
                    },
                }
            }
        }
    }
    pub fn sort_waiting_metrics(&mut self){
        self.metrics_to_get.sort_by_key(| k | k.time_remaining_before_next_measure);
    }

    pub fn get_sleep_time(&mut self) -> u128 {
       self.sort_waiting_metrics();
       println!("shortest time remaining : {}", (self.metrics_to_get[0].clone().time_remaining_before_next_measure*1000000 ) as u128 );
       (self.metrics_to_get[0].clone().time_remaining_before_next_measure * 1000000) as u128
    }

    // returns HashMap<backend_name, HashMap<job_id, Vec<Metric>>>
    pub fn get_metrics_to_collect_now(&mut self, delta_t: i64) -> HashMap<String, HashMap<i32, Vec<Metric>>>{
        let mut list_metrics:HashMap<String, HashMap<i32, Vec<Metric>>>=HashMap::new();
        for i in 0..self.metrics_to_get.len() {
            self.metrics_to_get[i].time_remaining_before_next_measure-=delta_t;
            if self.metrics_to_get[i].time_remaining_before_next_measure <= 0 {
                // add backend
                if list_metrics.get_mut(&self.metrics_to_get[i].backend_name).is_none() {
                    list_metrics.insert(self.metrics_to_get[i].backend_name.clone(), HashMap::new());
                }
                let tmp_back=list_metrics.get_mut(&self.metrics_to_get[i].backend_name).unwrap();
                // add job_id
                if tmp_back.get_mut(&self.metrics_to_get[i].job_id).is_none() {
                    tmp_back.insert(self.metrics_to_get[i].job_id, Vec::new());
                }
                let tmp_job=tmp_back.get_mut(&self.metrics_to_get[i].job_id).unwrap();
                tmp_job.push(self.metrics_to_get[i].clone());
                self.metrics_to_get[i].time_remaining_before_next_measure = if self.metrics_to_get[i].sampling_period == -1. { self.sample_period } else { (self.metrics_to_get[i].sampling_period * 1000.) as i64 };
            }
        }
        list_metrics
    }
    pub fn update_measurement(&self, job_id: i32, metrics: Vec<MetricValues>){
        let (_hostname, _timestamp, _version, measures)=self.last_measurement.get(&job_id).unwrap().clone();
        let i=0;
        for mut measure in measures {
              if measure.backend_name==metrics[i].backend_name && measure.metric_names==metrics[i].metric_names {
                  measure.metric_values=metrics[i].metric_values.clone();
              }
        }
    }
}
