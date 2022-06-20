extern crate zmq;
extern crate serde;
extern crate rmp_serde as rmps;

use std::collections::HashMap;
extern crate rmp_serialize;
extern crate rustc_serialize;

use serde::{Deserialize, Serialize};
use rmps::{Deserializer, Serializer};
use crate::backends::metric::MetricValues;

pub struct ZmqSender {
    sender: zmq::Socket, // sends counters to colmet-collector
    receiver: zmq::Socket, // receives user configuration
    //backends: &'a Vec<Box<dyn Backend>>,
}

impl ZmqSender {

    pub fn init() -> ZmqSender {
        let context = zmq::Context::new();
        let sender = context.socket(zmq::PUSH).unwrap();
        let receiver = context.socket(zmq::PULL).unwrap();
        ZmqSender{sender, receiver}//, backends}
    }

    pub fn open(&self, uri:&str, linger:i32, high_watermark:i32){
        self.sender.connect(uri).unwrap();
        self.sender.set_linger(linger).unwrap();
        self.sender.set_sndhwm(high_watermark).unwrap();
        self.receiver.bind("tcp://0.0.0.0:5557").unwrap();
        self.receiver.set_linger(linger).unwrap();
        self.receiver.set_rcvhwm(high_watermark).unwrap();

    }

    pub fn send_metrics(&self, metrics: HashMap<i32, (String, i64, i64, Vec<MetricValues>)>) {
        let mut buf = Vec::new();
        match metrics.serialize(&mut Serializer::new(&mut buf)){
            Err(e) => debug!("{}", e),
            Ok(_t) => ()
        }
        //        let mut de = Deserializer::new(&buf[..]);
        //        let res: HashMap<i32, (String, i64, Vec<(String, Vec<i32>, Vec<i64>)>)> = Deserialize::deserialize(&mut de).unwrap();
        self.sender.send(buf, 0).unwrap();
    }

    // receive message containing a new config for colmet, change sample period and metrics collected by backends (only perfhw at the moment)
    pub fn receive_config(&self) -> Option<HashMap<String,String>> {
        let mut msg = zmq::Message::new();
        let mut item=[self.receiver.as_poll_item(zmq::POLLIN)];
        zmq::poll(&mut item, 0).unwrap();
        if item[0].is_readable() && self.receiver.recv(&mut msg, 0).is_ok() {
            let mut deserializer = Deserializer::new(&msg[..]);
            let config:HashMap<String, String> = Deserialize::deserialize(&mut deserializer).unwrap();
            debug!("config {:#?}", config);
            Some(config)
        }
        else{
            None
        }
    }
}
