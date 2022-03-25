extern crate gethostname;

use std::collections::HashMap;
use std::sync::Arc;

use crate::backends::metric::Metric;
use crate::backends::metric::MetricValues;
use crate::backends::Backend;
use crate::cgroup_manager::CgroupManager;

use std::slice;

extern crate libc;

pub struct PerfhwBackend {
    pub backend_name: String,
    cgroup_manager: Arc<CgroupManager>,
}

impl PerfhwBackend {
    pub fn new(cgroup_manager: Arc<CgroupManager>) -> PerfhwBackend { // this function is almost the same for all backends but there is no inheritance in rust, use composition ?
        let backend_name = "Perfhw".to_string();

        PerfhwBackend { backend_name, cgroup_manager }
    }
}

impl Backend for PerfhwBackend {
    fn say_hello(&self) {
        println!("hello my name is perfhw backend");
    }

    fn get_backend_name(&self) -> String{
        return self.backend_name.clone();
    }

    fn return_values(&self, mut metrics_to_get: HashMap<i32, Vec<Metric>>) -> HashMap<i32, MetricValues> {
        let mut ret:HashMap<i32, MetricValues>=HashMap::new();
        let cgroups = self.cgroup_manager.get_cgroups();
        debug!("cgroup: {:#?}", cgroups);

        for (cgroup_id, cgroup_name) in cgroups {
            debug!(
                "Getting cgroup name:= {}, with id:={}",
                cgroup_name, cgroup_id
            );
            let cgroup_name_string = format!("/oar/{}{}", cgroup_name, "\0").to_string();
            let cgroup_name = cgroup_name_string.as_ptr();
            let mut metric_names="".to_string();
            for m in metrics_to_get.get_mut(&cgroup_id).unwrap(){
                 metric_names= format!("{} {}",  metric_names , m.metric_name);
            }
            metric_names = format!("{}{}", metric_names, "\0");
            debug!("Getting metrics: {}", metric_names);
            let mut metric_values = get_metric_values(
                cgroup_name,
                metric_names.as_ptr(),
                metrics_to_get.get(&cgroup_id).unwrap().len(),
            ); // https://doc.rust-lang.org/std/ffi/struct.CString.html this type seems more appropriate but I cant get it to work
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
        ret
    }

}

fn get_metric_values(cgroup_name: *const u8, metrics_to_get: *const u8, nb_metrics_to_get: usize) -> Vec<i64> {
    #[link(name = "perf_hw", kind="static")]
    extern {
        fn init_cgroup(cgroup_name: *const u8, metrics: *const u8) -> i32;
    }

    extern "C" {
        fn get_counters(values: *mut i64, cgroup_name: *const u8) -> i32;
    }
    // calling init_cgroup does nothing if the cgroup is already in the list
    let _res = unsafe {init_cgroup(cgroup_name, metrics_to_get)};
    let mut buffer = Vec::with_capacity(nb_metrics_to_get*64 as usize);
    let buffer_ptr = buffer.as_mut_ptr();
    let _res2 = unsafe {get_counters(buffer_ptr, cgroup_name)};
    let metric_values = unsafe { slice::from_raw_parts(buffer_ptr, nb_metrics_to_get).to_vec()};
    metric_values
}
