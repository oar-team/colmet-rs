#!/bin/bash

echo "input file :"
read filename

echo "Launch node agent ?"
read blbl

cargo run -- -f $filename --enable-perfhw true 
