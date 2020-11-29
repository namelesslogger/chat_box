#[allow(dead_code)]
mod util;
use hex;
use std::{
    sync::mpsc::{SyncSender, Receiver, channel, sync_channel},
    thread,
    env,
    collections::HashMap,
    io::{stdout, stdin, Write}
};
use rand::{thread_rng, Rng};
use termion::{event::Key, input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use crypto_box::{Box, PublicKey, SecretKey, aead::Aead};


struct ChatSender {
    public_key: [u8; 32],
    client_port: u16,
    username: Vec<u8>,
    private_key: crypto_box::SecretKey,
    client_recieve: Receiver<[u8; 32]>
}

struct ChatListener {
    server_port: u16,
    server_send: SyncSender<[u8; 32]>
}

impl ChatSender {
    fn send_task(self) {
        let client_connection = String::from("tcp://localhost:") + &self.client_port.to_string();
        let context = zmq::Context::new();
        let client = context.socket(zmq::DEALER).unwrap();
        let (tx, rx) = channel::<String>();

        client
            .set_identity(&self.username)
            .expect("failed setting client id");
        client
            .connect(&client_connection)
            .expect("failed connecting client");

        client
            .send(&hex::encode(self.public_key), 0)
            .expect("client failed sending request");

        let friend_public_key = PublicKey::from(self.client_recieve.recv().unwrap());
        let friend_box = Box::new(&friend_public_key, &self.private_key);

        thread::spawn(move || loop { 
            let mut s=String::new();
            let _=stdout().flush();
            stdin().read_line(&mut s).expect("What");  
            
            tx.send(s).unwrap();
        });

        loop {
            let request = match rx.try_recv() {
                Ok(s) => s,
                Err(_e) => String::from("")
            };
            
            if ! request.is_empty() {
                let mut rng = rand::thread_rng();
                let nonce = crypto_box::generate_nonce(&mut rng);
                let ciphertext = friend_box.encrypt(&nonce, request.as_bytes()).unwrap();
                client
                    .send(&hex::encode(ciphertext), 0)
                    .expect("client failed sending request");
            }   
        }
    }
}

impl ChatListener {
    fn listen_task(self) {
        let server_connection = String::from("tcp://*:") + &self.server_port.to_string();
        let context = zmq::Context::new();
        let frontend = context.socket(zmq::ROUTER).unwrap();
        match frontend.bind(&server_connection) {
            Ok(_) => println!("front end port bound"),
            Err(_e) => {
                println!("Bailing out, port already bound");
            },
        }

        let backend = context.socket(zmq::DEALER).unwrap();
        backend
            .bind("inproc://backend")
            .expect("server failed binding backend");
        
        let ctx = context.clone();
        thread::spawn(move || self.listener(&ctx));
        
        zmq::proxy(&frontend, &backend).expect("server failed proxying");
    }

    fn listener(self, context: &zmq::Context) {
        let worker = context.socket(zmq::DEALER).unwrap();
        worker
            .connect("inproc://backend")
            .expect("worker failed to connect to backend");
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
                let fried_public_key_bytes = match hex::decode(message) {
                    Ok(s) => s,
                    Err(_e) => {
                        println!("Whoops,invalid hex character encountered..");
                        Vec::<u8>::new()
                    },
                };
    
                let mut fried_public_key_bytes_array: [u8; 32] = [0; 32];
                for n in 0..32 {
                    fried_public_key_bytes_array[n] = fried_public_key_bytes[n];
                }
                // pass key to client
                self.server_send.send(fried_public_key_bytes_array);
            } else {
                println!("User: {}: \n -> {:?}", identity, message);
            
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
    let (server_send, client_recieve) = sync_channel::<[u8; 32]>(1);

    thread::spawn(move ||{ 
        let listen = ChatListener { 
            server_port: server_port,
            server_send: server_send
        };
        
        listen.listen_task(); 
    });

    let chatter = ChatSender { 
        client_port: client_port, 
        public_key: my_public_key_bytes, 
        username: username.as_bytes().to_vec(),
        private_key: my_secret_key,
        client_recieve: client_recieve
    };

    chatter.send_task();
}