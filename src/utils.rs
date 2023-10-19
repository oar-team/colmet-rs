use std::path::Path;
use std::path::PathBuf;

use inotify::{EventMask, Inotify, WatchMask};

pub fn wait_file(filename: &String, wait: bool) {
    if !Path::new(filename).exists() {
        debug!("filename does not exist {}", filename);
        if !wait {
            error!("filename does not exist {}", filename);
            std::process::exit(1);
        } else {
            let mut inotify = Inotify::init().expect("Failed to initialize inotify");
            let split_path = filename.split('/');
            let mut path = PathBuf::from("/");
            let mut flag = false;

            debug!("Waiting filename's creation {}: ", filename);
            // Can be a recursive process
            for next in split_path {
                let dir = path.clone();
                path.push(next);

                if !Path::new(&path).exists() {
                    inotify
                        .add_watch(dir, WatchMask::CREATE)
                        .expect("Failed to add inotify watch");
                    let mut buffer = [0u8; 4096];
                    while !Path::new(&path).exists() {
                        debug!("Waiting !");
                        let events = inotify
                            .read_events_blocking(&mut buffer)
                            .expect("Failed to read inotify events");
                        for event in events {
                            if event.mask.contains(EventMask::ISDIR) {
                                debug!("Directory created: {:?}", event.name);
                                if Path::new(&filename).exists() {
                                    flag = true;
                                }
                            }
                        }
                    }
                }
                if flag {
                    break;
                }
            }
            inotify.close().expect("Failed to close inotify instance");
        }
    }
}

pub fn round_sampling(default_period: i64, met_period:f32) -> f32{
    let metric_period=(met_period*1000.) as i64;
    let res:i64;

    if metric_period > default_period {
        if metric_period%default_period==0{
            res=metric_period;
        }else{
            let floor=default_period*(metric_period/default_period);
            let roundup=default_period*((metric_period/default_period)+1);
            if i64::abs(floor - metric_period) < roundup - metric_period {
                res=floor;
            }else{
                res=roundup;
            }
        }
    }else{
        if default_period%metric_period==0{
            res=metric_period;
        }else{
            let roundup=default_period/(default_period/metric_period);
            let floor=default_period/((default_period/metric_period)+1);
            if i64::abs(floor - metric_period) < roundup - metric_period {
                res=floor;
            }else{
                res=roundup;
            }
        }
    }
    (res as f32)/1000.
}
