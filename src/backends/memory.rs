extern crate gethostname;

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::rc::Rc;
use std::sync::Arc;

use crate::backends::metric::Metric;
use crate::backends::metric::MetricValues;
use crate::backends::Backend;
use crate::cgroup_manager::CgroupManager;
use crate::utils::wait_file;

pub struct MemoryBackend {
    pub backend_name: String,
    cgroup_manager: Arc<CgroupManager>,
    metrics: Rc<RefCell<HashMap<i32, MetricValues>>>,
}

impl MemoryBackend {
    pub fn new(cgroup_manager: Arc<CgroupManager>) -> MemoryBackend {
        // this function is almost the same for all backends but there is no inheritance in rust, use composition ?
        let backend_name = "memory".to_string();
        let metrics = Rc::new(RefCell::new(HashMap::new()));

        let cgroups = cgroup_manager.get_cgroups();
        debug!("mais putain {:#?}", cgroups);

        for (cgroup_id, cgroup_name) in cgroups {
            debug!("{} : {}", cgroup_id, cgroup_name);
            let filename:String;
            if cgroup_manager.cgroup_path_suffix.ne(""){
                filename=format!("{}/memory{}/{}/memory.stat", cgroup_manager.cgroup_root_path, cgroup_manager.cgroup_path_suffix, cgroup_name);
            }else {
               filename=format!("{}/memory/memory.stat", cgroup_manager.cgroup_root_path); 
            }
            wait_file(&filename, true);

            let metric_names = get_metric_names(filename);

            let metric = MetricValues {
                job_id: cgroup_id,
                backend_name: backend_name.clone(),
                metric_names,
                metric_values: Vec::new(),
            };
            (*metrics).borrow_mut().insert(cgroup_id, metric);
        }
        MemoryBackend {
            backend_name,
            cgroup_manager,
            metrics,
        }
    }
}

impl Backend for MemoryBackend {
    fn say_hello(&self) {
        println!("hello my name is memory backend");
    }
    fn get_backend_name(&self) -> String {
        return self.backend_name.clone();
    }
    
    fn return_values(&self, mut metrics_to_get: HashMap<i32, Vec<Metric>>) -> HashMap<i32, MetricValues> {
        let mut ret:HashMap<i32, MetricValues>=HashMap::new();
        let cgroups = self.cgroup_manager.get_cgroups();
        debug!("cgroup: {:#?}", cgroups);

        for (cgroup_id, cgroup_name) in cgroups {
            if metrics_to_get.get(&(-1)).is_some() {
                if metrics_to_get.get(&cgroup_id).is_none(){
                    let v:Vec<Metric>=Vec::new();
                    metrics_to_get.insert(cgroup_id, v);
                }
                for m in metrics_to_get.get(&(-1)).unwrap().clone() {
                    metrics_to_get.get_mut(&cgroup_id).unwrap().push(m);
                }
            }
            if metrics_to_get.get(&cgroup_id).is_some() {
                let filename = format!(
                    "{}/memory{}/{}/memory.stat",
                    self.cgroup_manager.cgroup_root_path,
                    self.cgroup_manager.cgroup_path_suffix,
                    cgroup_name
                );

                wait_file(&filename, true);
                debug!("metrics: {:#?}", self.metrics);

                let mut metric_values = get_metric_values(&filename, metrics_to_get.get(&cgroup_id).unwrap().clone());

                if ret.contains_key(&cgroup_id) {
                    ret.get_mut(&cgroup_id).unwrap().metric_values.append(metric_values.as_mut());
                } else {
                    let mut m_names: Vec<String>=Vec::new();
                    for m in metrics_to_get.get_mut(&cgroup_id).unwrap() {
                        m_names.push(m.metric_name.clone());
                    }
                    let metric = MetricValues {
                        job_id: cgroup_id,
                        backend_name: self.backend_name.clone(),
                        metric_names: m_names,
                        metric_values,
                    };
                    ret.insert(cgroup_id, metric);
                }
            }
        }
        ret  
    }
}

fn get_metric_names(filename: String) -> Vec<String> {
    debug!("get metric: getting file: {}", filename);
    let mut file = File::open(filename).expect("Cannot open file");
    let mut content = String::new();
    file.read_to_string(&mut content)
        .expect("Cannot get file content");
    let lines: Vec<&str> = content.split("\n").collect();
    let mut res: Vec<String> = Vec::new();
    for i in 0..lines.len() - 1 {
        let line = lines[i];
        let tmp1 = line.to_string();
        let tmp2: Vec<&str> = tmp1.split(" ").collect();
        res.push(tmp2[0].to_string());
    }
    let metric_names = res[..res.len()].to_vec();

    metric_names
}

fn get_metric_values(filename: &String, metrics_to_get: Vec<Metric>) -> Vec<i64> {
    let mut file = File::open(filename).unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    let lines: Vec<&str> = content.split("\n").collect();
    let mut res: Vec<i64> = Vec::new();
    let mut h:HashMap<String, String>=HashMap::new();
    // not really efficient. TODO: find an appropriate data structure to efficiently recover the
    // data specified in metrics_to_get
    for i in 0..lines.len() - 1 {
        let line = lines[i];
        let tmp1 = line.to_string();
        let tmp2: Vec<&str> = tmp1.split(" ").collect();
        h.insert(tmp2[0].to_string(), tmp2[1].to_string());
    }
    for m in metrics_to_get {
        res.push(h.get_mut(&m.metric_name).unwrap().parse::<i64>().unwrap());
    }
    // why did he do this ???
    //let metric_values = res[..res.len()].to_vec();
    //metric_values
    res
}
