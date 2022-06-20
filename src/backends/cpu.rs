extern crate gethostname;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;

use crate::backends::metric::Metric;
use crate::backends::metric::MetricValues;
use crate::backends::Backend;
use crate::cgroup_manager::CgroupManager;

use crate::utils::wait_file;

pub struct CpuBackend {
    pub backend_name: String,
    cgroup_manager: Arc<CgroupManager>,
}

impl CpuBackend {
    pub fn new(cgroup_manager: Arc<CgroupManager>) -> CpuBackend {
        // this function is almost the same for all backends but there is no inheritance in rust, use composition ?
        let backend_name = "cpu".to_string();
        CpuBackend {
            backend_name,
            cgroup_manager,
        }
    }
}

impl Backend for CpuBackend {
    fn say_hello(&self) {
        println!("hello my name is cpu backend");
    }

    fn get_backend_name(&self) -> String{
        self.backend_name.clone()
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
                    "{}/cpu{}/{}/cpu.stat",
                    self.cgroup_manager.cgroup_root_path,
                    self.cgroup_manager.cgroup_path_suffix,
                    cgroup_name
                );

                wait_file(&filename, true);

                let metric_values = get_metric_values(&filename, metrics_to_get.get(&cgroup_id).unwrap().clone());

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
        ret  
    }
}

fn get_metric_values(filename: &String, metrics_to_get: Vec<Metric>) -> Vec<i64> {
    let mut file = File::open(filename).unwrap();
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    let lines: Vec<&str> = content.split('\n').collect();
    let mut res: Vec<i64> = Vec::new();
    let mut h:HashMap<String, String>=HashMap::new();
    // not really efficient. TODO: find an appropriate data structure to efficiently recover the
    // data specified in metrics_to_get
    for line in lines.iter().take(lines.len() - 1) {
        let tmp1 = line.to_string();
        let tmp2: Vec<&str> = tmp1.split(' ').collect();
        h.insert(tmp2[0].to_string(), tmp2[1].to_string());
    }
    for m in metrics_to_get {
        res.push(h.get_mut(&m.metric_name).unwrap().parse::<i64>().unwrap());
    }
    res
}
