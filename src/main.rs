#[allow(dead_code)]
mod util;
use hex;
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

use crypto_box::{Box, PublicKey, SecretKey, aead::Aead};
use rand;


fn main() {
    let args: Vec<String> = env::args().collect();
    run(&args[1], &args[2], &args[3]);
}

fn run(myport: &String, friend_port:&String, username: &String) {
    let client_connection = String::from("tcp://localhost:") + friend_port;
    let server_connection = String::from("tcp://*:") + myport;

    let mut rng = thread_rng();
    let my_secret_key = SecretKey::generate(&mut rng);
    let my_public_key_bytes = my_secret_key.public_key().as_bytes().clone();

    let my_public_key_hex = hex::encode(my_public_key_bytes);
    let my_public_key_hex_clone = hex::encode(my_public_key_bytes);

    thread::spawn(move ||{ server_task(&server_connection, my_secret_key); });
    client_task(&client_connection, username, &my_public_key_hex);
}


fn client_task(client_port: &str, username: &str, my_public_key_hex: &str) {
    let context = zmq::Context::new();
    let client = context.socket(zmq::DEALER).unwrap();
    let (tx, rx) = channel::<String>();

    client
        .set_identity(username.as_bytes())
        .expect("failed setting client id");
    client
        .connect(client_port)
        .expect("failed connecting client");

    client
        .send(my_public_key_hex, 0)
        .expect("client failed sending request");

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

fn server_task(server_port: &str, my_secret_key: SecretKey) {
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
    thread::spawn(move || server_worker(&ctx, &my_secret_key));
    
    zmq::proxy(&frontend, &backend).expect("server failed proxying");
}

fn server_worker(context: &zmq::Context, my_secret_key: &SecretKey) {
    let worker = context.socket(zmq::DEALER).unwrap();
    worker
        .connect("inproc://backend")
        .expect("worker failed to connect to backend");
    let mut rng = thread_rng();
    let mut message_counter = 0;
    loop {
        let identity = worker
            .recv_string(0)
            .expect("worker failed receiving identity")
            .unwrap();
        let message = worker
            .recv_string(0)
            .expect("worker failed receiving message")
            .unwrap();

        if message_counter == 0 {
            // message
            let fried_public_key_bytes = hex::decode(message).unwrap();

            let mut fried_public_key_bytes_array: [u8; 32] = [0; 32];
            for n in 0..32 {
                fried_public_key_bytes_array[n] = fried_public_key_bytes[n];
            }
            let mut rng = thread_rng();
            let nonce = crypto_box::generate_nonce(&mut rng);
            let fried_public_key = PublicKey::from(fried_public_key_bytes_array);
            println!("{:?}", fried_public_key);
            let my_box = Box::new(&fried_public_key, &my_secret_key);

            let plaintext = b"Top secret message we're encrypting";

            let ciphertext = my_box.encrypt(&nonce, &plaintext[..]).unwrap();

            println!("{:?}", ciphertext);
        } else {
            println!("User: {}: \n -> {}", identity, message);
        
            worker
                .send(&identity, zmq::SNDMORE)
                .expect("worker failed sending identity");
            worker
                .send(&message, 0)
                .expect("worker failed sending message");
        }
        message_counter += 1;
    }
}