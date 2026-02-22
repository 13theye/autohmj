// src/osc/osc_module.rs
//
//

use nannou_osc as osc;
use std::error::Error;

use crate::settings::OscSendConfig;

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
        println!("OSC Sender sending to {}:{}", target_addr, target_port);

        Ok(Self {
            sender,
            target_addr,
            target_port,
        })
    }

    /// - Sends choseong as 4352-4370
    /// - Sends jungseong as 4449-4469
    /// - Sends jeongseong as 4520-4546
    /// - OR compatability range: 12593-12686
    /// - Sentinel: -1 in jungseong and jeongsong if bare jamo
    /// - 0 in jeongsong if no batchim
    pub fn send_seq(&self, id: &str, choseong: i32, jungseong: i32, jeongseong: i32) {
        let addr = "/hunmin/seq".to_string();
        let args = vec![
            osc::Type::String(id.to_string()),
            osc::Type::Int(choseong),
            osc::Type::Int(jungseong),
            osc::Type::Int(jeongseong),
        ];
        self.sender
            .send((addr, args), (self.target_addr.as_str(), self.target_port))
            .ok();
    }

    /// - Sends punctuation as 33-47; 58-64; 91-96; 123-126
    pub fn send_punctuation(&self, id: &str, punctuation: i32) {
        let addr = "/hunmin/punc".to_string();
        let args = vec![
            osc::Type::String(id.to_string()),
            osc::Type::Int(punctuation),
        ];
        self.sender
            .send((addr, args), (self.target_addr.as_str(), self.target_port))
            .ok();
    }

    /// - Sends ASCII digits (0-9) as 48-57
    /// - ASCII uppercase letters (A-Z) as 65-90
    /// - ASCII lowercase letters (a-z) as 97-122
    pub fn send_alphanumeric(&self, id: &str, letter: i32) {
        let addr = "/hunmin/alpha".to_string();
        let args = vec![osc::Type::String(id.to_string()), osc::Type::Int(letter)];
        self.sender
            .send((addr, args), (self.target_addr.as_str(), self.target_port))
            .ok();
    }

    pub fn send_num_letters(&self, id: &str, num_letters: i32) {
        let addr = "/hunmin/numLetters".to_string();
        let args = vec![
            osc::Type::String(id.to_string()),
            osc::Type::Int(num_letters),
        ];
        self.sender
            .send((addr, args), (self.target_addr.as_str(), self.target_port))
            .ok();
    }

    pub fn send_ai_requested(&self, id: &str) {
        let addr = "/hunmin/AIRequested".to_string();
        let args = vec![osc::Type::String(id.to_string())];
        self.sender
            .send((addr, args), (self.target_addr.as_str(), self.target_port))
            .ok();
    }

    pub fn send_ai_typing(&self, id: &str) {
        let addr = "/hunmin/AITyping".to_string();
        let args = vec![osc::Type::String(id.to_string())];
        self.sender
            .send((addr, args), (self.target_addr.as_str(), self.target_port))
            .ok();
    }

    pub fn send_ai_finished(&self, id: &str) {
        let addr = "/hunmin/AIFinished".to_string();
        let args = vec![osc::Type::String(id.to_string())];
        self.sender
            .send((addr, args), (self.target_addr.as_str(), self.target_port))
            .ok();
    }
}
