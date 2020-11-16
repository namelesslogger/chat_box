#[allow(dead_code)]
mod util;

use std::{
    error::Error, 
    io::{stdout, stdin, Write}};
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use chrono::{DateTime, Local};
use std::sync::mpsc::{sync_channel, channel};
use unicode_width::UnicodeWidthStr;
use std::io::prelude::*;
use std::thread;
use std::time::Duration;

fn main() {
    run();
}

fn run() {
    thread::spawn(||{
        server();
    });
    client();
}

fn client() {
    let context = zmq::Context::new();

    let sender = context.socket(zmq::PUSH).unwrap();
    let reciever = context.socket(zmq::PULL).unwrap();

    assert!(sender.connect("tcp://localhost:5555").is_ok());
    assert!(reciever.connect("tcp://localhost:5556").is_ok());
    
    // waits for input
    thread::spawn(move || loop { 
        let mut s=String::new();
        let _=stdout().flush();
        stdin().read_line(&mut s).expect("What");   
        
        sender.send(zmq::Message::from(&s), 0).unwrap();
    });

    let mut msg = zmq::Message::new();
    loop {
        let mut pulled_messages: Vec<String> = vec![];
        reciever.recv(&mut msg, 0).unwrap();
        let message_txt = msg.as_str().unwrap();

        println!("{}", message_txt)
        pulled_messages.push(String::from(message_txt));
    }
}

fn server() {
    let context = zmq::Context::new();

    let reciever = context.socket(zmq::PULL).unwrap();
    let responder = context.socket(zmq::PUSH).unwrap();

    assert!(reciever.bind("tcp://*:5555").is_ok());
    assert!(responder.bind("tcp://*:5556").is_ok());

    let mut msg = zmq::Message::new();
    loop {
        reciever.recv(&mut msg, 0).unwrap();
        let message_txt = msg.as_str().unwrap();
        responder.send(zmq::Message::from(message_txt), 0).unwrap();
    }
}
