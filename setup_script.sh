#!/bin/bash

cd colmet-rs
git pull

sudo-g5k ln -s /sys/fs/cgroup/perf_event /dev/oar_cgroups_links/
sudo-g5k mkdir -p /dev/oar_cgroups_links/perf_event/$OAR_CPUSET
echo $$ | sudo-g5k tee -a /dev/oar_cgroups_links/perf_event$OAR_CPUSET/tasks

sudo-g5k apt install libzmq3-dev -y > /dev/null 2> /dev/null

cargo build
