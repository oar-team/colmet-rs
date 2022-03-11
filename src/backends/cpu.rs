extern crate gethostname;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;

use crate::backends::metric::Metric;
use crate::backends::Backend;
use crate::cgroup_manager::CgroupManager;

use crate::utils::wait_file;
use std::cell::RefCell;

use std::rc::Rc;

pub struct CpuBackend {
    pub backend_name: String,
    cgroup_manager: Arc<CgroupManager>,
    metrics: Rc<RefCell<HashMap<i32, Metric>>>,
    metrics_to_get: Rc<RefCell<Vec<(f32,String)>>>,
}

impl CpuBackend {
    pub fn new(cgroup_manager: Arc<CgroupManager>) -> CpuBackend {
        // this function is almost the same for all backends but there is no inheritance in rust, use composition ?
        let backend_name = "Cpu".to_string();

        let metrics = Rc::new(RefCell::new(HashMap::new()));
        
        let metrics_to_get = Rc::new(RefCell::new(metrics_to_get.clone()));

        for (cgroup_id, cgroup_name) in cgroup_manager.get_cgroups() {
            let filename = format!(
                "{}/cpu{}/{}/cpu.stat",
                cgroup_manager.cgroup_root_path, cgroup_manager.cgroup_path_suffix, cgroup_name
            );
            let metric_names = get_metric_names(&filename);
            let metric = Metric {
                job_id: cgroup_id,
                backend_name: backend_name.clone(),
                metric_names,
                metric_values: None,
            };
            (*metrics).borrow_mut().insert(cgroup_id, metric);
        }
        CpuBackend {
            backend_name,
            cgroup_manager,
            metrics,
            metrics_to_get,
        }
    }
}

impl Backend for CpuBackend {
    fn say_hello(&self) {
        println!("hello my name is cpu backend");
    }

    fn open(&self) {}

    fn close(&self) {}

    fn get_backend_name(&self) -> String{
        return self.backend_name.clone();
    }

    fn get_metrics(&self) -> HashMap<i32, Metric> {
        let cgroups = self.cgroup_manager.get_cgroups();
        debug!("cgroup: {:#?}", cgroups);

        for (cgroup_id, cgroup_name) in cgroups {
            let filename = format!(
                "{}/cpu{}/{}/cpu.stat",
                self.cgroup_manager.cgroup_root_path,
                self.cgroup_manager.cgroup_path_suffix,
                cgroup_name
            );

            wait_file(&filename, true);
            debug!("metrics: {:#?}", self.metrics);

            let metric_values = get_metric_values(&filename);

            let mut borrowed_metrics = self.metrics.borrow_mut();
            if (borrowed_metrics.contains_key(&cgroup_id)) {
                borrowed_metrics.get_mut(&cgroup_id).unwrap().metric_values = Some(metric_values);
            } else {
                let metric_names = get_metric_names(&filename);
                let metric = Metric {
                    job_id: cgroup_id,
                    backend_name: self.backend_name.clone(),
                    metric_names,
                    metric_values: Some(metric_values),
                };
                borrowed_metrics.insert(cgroup_id, metric);
            }
        }
//        println!("new metric {:#?}", self.metrics.clone());
        (*self.metrics).borrow_mut().clone()
    }

    fn set_metrics_to_get(& self, _metrics_to_get: Vec<String>){
        ()
    }
}

fn get_metric_names(filename: &String) -> Vec<String> {
    debug!("openning {}", &filename);
    let mut file = File::open(filename).unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    let lines: Vec<&str> = content.split("\n").collect();
    let mut res: Vec<String> = Vec::new();
    for i in 0..lines.len() - 1 {
        let line = lines[i];
        let tmp1 = line.to_string();
        let tmp2: Vec<&str> = tmp1.split(" ").collect();
        res.push(tmp2[0].to_string());
    }
    //    let metric_names = res[..res.len()].to_vec().into_iter();
    let metric_names = res[..res.len()].to_vec();

    metric_names
}

fn get_metric_values(filename: &String) -> Vec<i64> {
    let mut file = File::open(filename).unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    let lines: Vec<&str> = content.split("\n").collect();
    let mut res: Vec<i64> = Vec::new();
    for i in 0..lines.len() - 1 {
        let line = lines[i];
        let tmp1 = line.to_string();
        let tmp2: Vec<&str> = tmp1.split(" ").collect();
        res.push(tmp2[1].parse::<i64>().unwrap());
    }
    let metric_values = res[..res.len()].to_vec();
    metric_values
}
