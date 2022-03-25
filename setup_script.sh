#!/bin/bash

sudo-g5k su

ln-s /sys/fs/cgroup/perf_event /dev/oar_cgroups_links/
mkdir -p /dev/oar_cgroups_links/perf_event/$OAR_CPUSET
echo $$ | sudo tee -a /dev/oar_cgroups_links/perf_event$OAR_CPUSET/tasks

apt install libzmq3-dev
