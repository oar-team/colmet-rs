#!/bin/bash

#echo "input file :"
#read filename

filename=t1_file.txt
echo "Launch node agent ?"
read blbl

cargo run -- -f $filename --enable-perfhw true > /dev/null 2> /dev/null & 
