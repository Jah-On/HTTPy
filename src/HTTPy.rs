use std::env;
use std::fs::File;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::collections::HashMap;
use std::string::String;
use tokio::{task, net::TcpListener, net::TcpStream, io};
use std::io::prelude::*;

pub struct HttpServer {
    sock:  TcpListener,
    ip:    IpAddr,
    port:  i16,
    root:  String,
    state: bool,
    time:  i32,                         // Timeout in ms before closing connection
    rqml:  usize,                       // Max length of a request
    fail:  i32,                         // Allowed fails before blocked -- FFI
    blkd:  Vec<IpAddr>,                 // Blocked addresses -- FFI
    flcnt: HashMap<IpAddr, i32>,        // Tally of failed/invalid requests for each IP
    get: HashMap<String, fn(String) -> String>, // Map of get functions
}

impl HttpServer {
    pub async fn new() -> Self {
        Self {
            sock:  TcpListener::bind("127.0.0.1:8080").await.unwrap(),
            ip:    IpAddr::V4(Ipv4Addr::new(127, 0, 0, 0)),
            port:  8080,
            root:  String::from(env::current_dir().unwrap().to_str().unwrap()),
            state: false,
            time:  1000,
            rqml:  4096,
            fail:  -1,
            blkd:  Vec::new(),
            flcnt: HashMap::new(),
            get:   HashMap::new(),
        }
    }
    pub async fn set_ip(&mut self, ip: &str){
        assert_eq!(ip.parse(), Ok(self.ip));
        let port = self.port;
        self.sock = TcpListener::bind(format!("{ip}:{port}")).await.unwrap();
    }
    pub async fn set_port(&mut self, port: i16){
        self.port = port;
        let ip = self.ip.to_string();
        self.sock = TcpListener::bind(format!("{ip}:{port}")).await.unwrap();
    }
    pub fn set_root_dir(&mut self, path: &str){
        self.root = String::from(path);
    }
    pub fn set_timeout(&mut self, ms: i32){
        self.time = ms;
    }
    pub fn set_max_request_length(&mut self, len: usize){
        self.rqml = len;
    }
    pub async fn run(&mut self) {
        self.state = true;
        while self.state {
            let stream = self.sock.accept().await;
            match stream {
                Ok((stream, addr)) => {
                    // client(stream, self.rqml.clone(), self.get.clone());
                    task::spawn(
                        client(stream, self.time.clone(), self.rqml.clone(), self.get.clone())
                    );
                }
                Err(e) => { println!("Connection failed!"); }
            }
        }
    }
    pub fn is_alive(&mut self) -> bool {
        return self.state.clone();
    }
    pub fn add_get(&mut self, path: &str, fn_: fn(String) -> String) {
        self.get.insert(String::from(path), fn_);
    }
}

async fn client(client: TcpStream, time: i32, rqml: usize, get: HashMap<String, fn(String) -> String>){
    let mut buf = String::new();
    let mut raw = [0; 1024];
    loop {
        match client.readable().await {
            Ok(_) => {}
            Err(e) => {println!("{e}");}
        }
        match client.try_read(& mut raw) {
            Ok(_) => {
                if raw.len() == 0 {
                    break;
                }
                else {
                    buf += &String::from_utf8_lossy(&raw);
                }
                if buf.len() >= rqml {
                    buf = buf[..rqml].to_owned();
                    break;
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(e) => {
                println!("{e}");
                return;
            }
        }
        match client.writable().await {
            Ok(_) => {}
            Err(e) => {println!("{e}");}
        }
        if &buf[..3] == "GET" {
            let path = &buf[buf.find(" ").unwrap() + 1 .. buf[buf.find(" ").unwrap() + 1 ..].find(" ").unwrap() + buf.find(" ").unwrap() + 1];
            if !get.contains_key(path) {
                match client.try_write("HTTP/1.1 404 NOT FOUND\r\n\r\n".as_bytes()) {
                    Ok(_) => {return;}
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {continue;}
                    Err(e) => {
                        println!("{e}");
                        return;
                    }
                }
            }
            match client.try_write(get[path]("".to_owned()).as_bytes()) {
                Ok(_) => {return;}
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {continue;}
                Err(e) => {
                    println!("{e}");
                    return;
                }
            }
        }
    }
}

pub fn ok() -> String {
    return "HTTP/1.1 200 OK\r\n\r\n".to_owned();
}

pub fn html(html: &str) -> String {
    return format!("HTTP/1.1 200 OK\r\n\r\n{html}").to_owned();
}

pub fn file(file: &str) -> String {
    let mut file = File::open(file).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    return format!("HTTP/1.1 200 OK\r\n\r\n{contents}").to_owned();
}