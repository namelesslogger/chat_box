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
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    run(&args[1]);
}

fn run(port: &String) {
    let client_connection = String::from("tcp://localhost:") + port;
    let server_connection = String::from("tcp://*:") + port;

    thread::spawn(move ||{
        server(&server_connection);
    });
    client(&client_connection);
}

fn client(client_port: &String) {
    let context = zmq::Context::new();

    let sender = context.socket(zmq::PUSH).unwrap();
    let reciever = context.socket(zmq::PULL).unwrap();

    assert!(sender.connect(client_port).is_ok());
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
        reciever.recv(&mut msg, 0).unwrap();
        let message_txt = msg.as_str().unwrap();

        println!("{}", message_txt);
    }
}

fn server(server_port: &String) {
    let context = zmq::Context::new();

    let reciever = context.socket(zmq::PULL).unwrap();
    let responder = context.socket(zmq::PUSH).unwrap();

    assert!(reciever.bind(server_port).is_ok());
    assert!(responder.bind("tcp://*:5556").is_ok());

    let mut msg = zmq::Message::new();
    loop {
        reciever.recv(&mut msg, 0).unwrap();
        let message_txt = msg.as_str().unwrap();
        responder.send(zmq::Message::from(message_txt), 0).unwrap();
    }
}
