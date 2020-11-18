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
use std::hash::{Hash, Hasher};
use rand::{thread_rng, Rng};

fn main() {
    let args: Vec<String> = env::args().collect();
    run(&args[1], &args[2], &args[3]);
}

fn run(myport: &String, friend_port:&String, username: &String) {
    let client_connection = String::from("tcp://localhost:") + friend_port;
    let server_connection = String::from("tcp://*:") + myport;

    thread::spawn(move ||{ server_task(&server_connection); });
    client_task(&client_connection, username);
}


fn client_task(client_port: &str, username: &str) {
    let context = zmq::Context::new();
    let client = context.socket(zmq::DEALER).unwrap();
    let mut rng = thread_rng();
    let identity = format!("{:04X}-{:04X}", rng.gen::<u16>(), rng.gen::<u16>());
    let (tx, rx) = channel::<String>();

    client
        .set_identity(username.as_bytes())
        .expect("failed setting client id");
    client
        .connect(client_port)
        .expect("failed connecting client");

        
    thread::spawn(move || loop { 
        let mut s=String::new();
        let _=stdout().flush();
        stdin().read_line(&mut s).expect("What");  
        
        tx.send(s).unwrap();
    });

    loop {
        if client.poll(zmq::POLLIN, 10).expect("client failed polling") > 0 {
            let msg = client
                .recv_multipart(0)
                .expect("client failed receivng response");
            //println!("Recieved: {}", std::str::from_utf8(&msg[msg.len() - 1]).unwrap());
        }

        let request = match rx.try_recv() {
            Ok(s) => s,
            Err(e) => String::from("")
        };
        
        //let request = format!("Sending a Wave!");
        if ! request.is_empty() {
            client
                .send(&request, 0)
                .expect("client failed sending request");
        }   
    }
}

fn server_task(server_port: &str) {
    let context = zmq::Context::new();
    let frontend = context.socket(zmq::ROUTER).unwrap();
    match frontend.bind(server_port) {
        Ok(_) => println!("front end port bound"),
        Err(e) => {
            println!("Bailing out, port already bound");
        },
    }

    let backend = context.socket(zmq::DEALER).unwrap();
    backend
        .bind("inproc://backend")
        .expect("server failed binding backend");
    
    let ctx = context.clone();
    thread::spawn(move || server_worker(&ctx));
    
    zmq::proxy(&frontend, &backend).expect("server failed proxying");
}

fn server_worker(context: &zmq::Context) {
    let worker = context.socket(zmq::DEALER).unwrap();
    worker
        .connect("inproc://backend")
        .expect("worker failed to connect to backend");
    let mut rng = thread_rng();

    loop {
        let identity = worker
            .recv_string(0)
            .expect("worker failed receiving identity")
            .unwrap();
        let message = worker
            .recv_string(0)
            .expect("worker failed receiving message")
            .unwrap();

        println!("User: {}: \n -> {}", identity, message);
        
        worker
            .send(&identity, zmq::SNDMORE)
            .expect("worker failed sending identity");
        worker
            .send(&message, 0)
            .expect("worker failed sending message");
    }
}