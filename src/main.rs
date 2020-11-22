#[allow(dead_code)]
mod util;
use hex;
use std::{
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
use std::sync::mpsc::channel;
use std::thread;
use std::env;
use rand::{thread_rng, Rng};

use crypto_box::{Box, PublicKey, SecretKey};
use rand;


struct Client {
    public_key: [u8; 32],
    client_port: u16
}

impl Client {
    fn client_task(self, username: &str) {
        let client_connection = String::from("tcp://localhost:") + &self.client_port.to_string();
        let context = zmq::Context::new();
        let client = context.socket(zmq::DEALER).unwrap();
        let (tx, rx) = channel::<String>();

        client
            .set_identity(username.as_bytes())
            .expect("failed setting client id");
        client
            .connect(&client_connection)
            .expect("failed connecting client");

        client
            .send(&hex::encode(self.public_key), 0)
            .expect("client failed sending request");

        thread::spawn(move || loop { 
            let mut s=String::new();
            let _=stdout().flush();
            stdin().read_line(&mut s).expect("What");  
            
            tx.send(s).unwrap();
        });

        loop {
            let request = match rx.try_recv() {
                Ok(s) => s,
                Err(e) => String::from("")
            };
            
            if ! request.is_empty() {
                client
                    .send(&request, 0)
                    .expect("client failed sending request");
            }   
        }
    }
}

struct Server {
    private_key: [u8; 32],
    server_port: u16
}

impl Server {
    fn server_task(self) {
        let server_connection = String::from("tcp://*:") + &self.server_port.to_string();
        let context = zmq::Context::new();
        let frontend = context.socket(zmq::ROUTER).unwrap();
        match frontend.bind(&server_connection) {
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
        thread::spawn(move || self.server_worker(&ctx));
        
        zmq::proxy(&frontend, &backend).expect("server failed proxying");
    }

    fn server_worker(self, context: &zmq::Context) {
        let worker = context.socket(zmq::DEALER).unwrap();
        worker
            .connect("inproc://backend")
            .expect("worker failed to connect to backend");
        let rng = thread_rng();
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
                //let my_box = Box::new(&fried_public_key, &my_secret_key);
    
                let plaintext = b"Top secret message we're encrypting";
    
                //let ciphertext = my_box.encrypt(&nonce, &plaintext[..]).unwrap();
    
                //println!("{:?}", ciphertext);
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
}

fn main() {
    let args: Vec<String> = env::args().collect();
    run(&args[1], &args[2], &args[3]);
}

fn run(client_port_str: &String, server_port_str: &String, username: &String) {
    // parse int from arguments
    let client_port: u16 = match client_port_str.parse() {
        Ok(n) => {
            n
        },
        Err(_) => {
            eprintln!("error: client port argument is not a valid number");
            return; // will replace with Err() handle response in main()
        },
    };
    let server_port: u16 = match server_port_str.parse() {
        Ok(n) => {
            n
        },
        Err(_) => {
            eprintln!("error: Server port argument s not a valid number");
            return;
        },
    };

    // generate keys
    let mut rng = thread_rng();
    let my_secret_key = SecretKey::generate(&mut rng);
    let my_public_key_bytes = my_secret_key.public_key().as_bytes().clone();

    thread::spawn(move ||{ 
        let server = Server { server_port: server_port, private_key: my_secret_key.to_bytes() };
        server.server_task(); 
    });
    let client = Client { client_port: client_port, public_key: my_public_key_bytes };
    client.client_task(username);
}