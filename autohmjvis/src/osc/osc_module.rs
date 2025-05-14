// src/osc/osc_module.rs
//
//

use nannou_osc as osc;
use std::error::Error;

use crate::config::OscSendConfig;

#[derive(Debug)]
pub enum OscCommand {
    Seq {
        id: i32,
        choseong: i32,
        jungseong: i32,
        jeongseong: i32,
    },
    NumLetters {
        id: i32,
        number: i32,
    },
}

pub struct OscSender {
    sender: osc::Sender,
    target_addr: String,
    target_port: u16,
}

impl OscSender {
    pub fn new(config: &OscSendConfig) -> Result<Self, Box<dyn Error>> {
        let target_addr = config.target_addr.to_owned();
        let target_port = config.target_port;
        let sender = osc::sender()?;

        Ok(Self {
            sender,
            target_addr,
            target_port,
        })
    }

    pub fn send_seq(&self, id: i32, choseong: i32, jungseong: i32, jeongseong: i32) {
        let addr = "/hunmin/seq".to_string();
        let args = vec![
            osc::Type::Int(id),
            osc::Type::Int(choseong),
            osc::Type::Int(jungseong),
            osc::Type::Int(jeongseong),
        ];
        self.sender
            .send((addr, args), (self.target_addr.as_str(), self.target_port))
            .ok();
    }

    pub fn send_num_letters(&self, id: i32, num_letters: i32) {
        let addr = "/hunmin/numLetters".to_string();
        let args = vec![osc::Type::Int(id), osc::Type::Int(num_letters)];
        self.sender
            .send((addr, args), (self.target_addr.as_str(), self.target_port))
            .ok();
    }
}
