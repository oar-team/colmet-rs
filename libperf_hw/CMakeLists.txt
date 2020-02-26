cmake_minimum_required(VERSION 3.0)
project(LibPerf_hw C)

add_library(perf_hw STATIC perf_hw.c)

install(TARGETS perf_hw DESTINATION .)
