extern crate inotify;
extern crate regex;

use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use inotify::{
    EventMask,
    Inotify,
    WatchMask,
};
use regex::Regex;


pub struct CgroupManager {
    pub cgroup_root_path: String, // cgroup root path
    pub cgroup_path_suffix: String,
    cgroups: Mutex<HashMap<i32, String>>, // cgroup corresponding to user jobs, keys : cgroup id, values : cgroup name
    regex_job_id: String, // regex to find the cpuset directory
}

impl CgroupManager {
    pub fn new(regex_job_id: String, cgroup_root_path: String, cgroup_path_suffix: String) -> Arc<CgroupManager> {
        let cgroups = Mutex::new(HashMap::new());
        let cgroup_path = format!("{}/cpuset{}", cgroup_root_path, cgroup_path_suffix);
        let res = Arc::new(CgroupManager { cgroup_root_path, cgroup_path_suffix, cgroups, regex_job_id });
        notify_jobs(Arc::clone(&res), cgroup_path);
        res
    }

    pub fn add_cgroup(&self, id: i32, name: String) {
        let mut map = self.cgroups.lock().unwrap();
        let mapmut = map.borrow_mut();
        mapmut.insert(id, name);
        debug!("cgroups after insertion: {:#?}", map);
    }

    pub fn remove_cgroup(&self, id: i32) {
        let mut map = self.cgroups.lock().unwrap();
        map.borrow_mut().remove(&id);
    }

    pub fn get_cgroups(&self) -> HashMap<i32, String> {
        self.cgroups.lock().unwrap().clone()
    }

    pub fn print_cgroups(&self) {
        println!("{:#?}", self.cgroups);
    }
}

// scan cpuset directory for changes and update cgroups list
pub fn notify_jobs(cgroup_manager: Arc<CgroupManager>, cgroup_path: String) {
    let regex_job_id = Regex::new(&cgroup_manager.regex_job_id).unwrap();
    debug!("{:#?}", cgroup_path);
    let cgroups = fs::read_dir(cgroup_path.clone()).unwrap();
    for cgroup in cgroups {
        let path = cgroup.unwrap().path();
        let cgroup_name = path.file_name().unwrap().to_str().unwrap();
        if let Some(v) = regex_job_id.find(cgroup_name) {
            cgroup_manager.add_cgroup(cgroup_name[v.start() + 1..v.end()].parse::<i32>().unwrap(), cgroup_name.to_string())
        }
    }

    let mut inotify = Inotify::init()
        .expect("Failed to initialize inotify");

    let current_dir = PathBuf::from(cgroup_path);

    inotify
        .add_watch(
            current_dir,
            WatchMask::MODIFY | WatchMask::CREATE | WatchMask::DELETE,
        )
        .expect("Failed to add inotify watch");

    debug!("Watching current directory for activity...");

    let mut buffer = [0u8; 4096];

    let _child = thread::spawn(move || loop {
        let events = inotify
            .read_events_blocking(&mut buffer)
            .expect("Failed to read inotify events");

        for event in events {
            cgroup_manager.print_cgroups();

            if event.mask.contains(EventMask::ISDIR) {
                let cgroup_name = event.name.unwrap().to_str().unwrap();

                if event.mask.contains(EventMask::CREATE) {
                    debug!("CREATE event!");
                    if let Some(v) = regex_job_id.find(cgroup_name) {
                        debug!("Add cgroup: {}", cgroup_name);
                        cgroup_manager.add_cgroup(
                            cgroup_name[v.start() + 1..v.end()].parse::<i32>().unwrap(),
                            cgroup_name.to_string(),
                        )
                    } else {
                        debug!("Nooop");
                    }
                } else if event.mask.contains(EventMask::DELETE)
                    && regex_job_id.is_match(cgroup_name)
                {
                    if let Some(v) = regex_job_id.find(cgroup_name) {
                        cgroup_manager.remove_cgroup(
                            cgroup_name[v.start() + 1..v.end()].parse::<i32>().unwrap(),
                        );
                    }
                }
            }
        }
    });
}
