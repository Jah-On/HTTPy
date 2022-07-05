use std::env;
use std::fs::{File, read_dir};
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
    get: HashMap<String, (fn(&str) -> String, String)>, // Map of get functions
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
            task::spawn(
                client(self.sock.accept().await.unwrap().0, self.time.clone(), self.rqml.clone(), self.get.clone())
            );
        }
    }
    pub fn is_alive(&mut self) -> bool {
        return self.state.clone();
    }
    pub fn add_get(&mut self, path: &str, fn_: fn(&str) -> String) {
        self.get.insert(String::from(path), (fn_, "".to_owned()));
    }
    pub fn handle_all_statics(&mut self){
        for _p in read_dir(&self.root).unwrap(){
            let path = _p.unwrap();
            let path_p = path.path();
            let path_s = path_p.to_str().unwrap();
            let mut path_r = String::new();
            path_r = String::from(path_s)[self.root.len() - 1..].to_owned();
            if path.file_type().unwrap().is_dir() {
                self.add_dir(path_s);
            }
            else if (path_r == "/index.html") || (path_r == "/index.php") {
                self.get.insert(String::from("/"), (|path: &str| -> String {return file(path);}, String::from(path_s)));
            } else {
                self.get.insert(path_r, (|path: &str| -> String {return file(path);}, String::from(path_s)));
            }
        }
        for elem in self.get.keys() {
            println!("{}", elem)
        }
    }
    fn add_dir(&mut self, dir: &str){
        for _p in read_dir(dir).unwrap(){
            let path = _p.unwrap();
            let path_p = path.path();
            let path_s = path_p.to_str().unwrap();
            let mut path_r = String::new();
            path_r = String::from(path_s)[self.root.len() - 1..].to_owned();
            if path.file_type().unwrap().is_dir() {
                self.add_dir(path_s);
            } else {
                self.get.insert(path_r, (|path: &str| -> String {return file(path);}, String::from(path_s)));
            }
        }
    }
}

async fn client(client: TcpStream, time: i32, rqml: usize, get: HashMap<String, (fn(&str) -> String, String)>){
    let mut buf_r: Vec<u8> = vec![];
    let mut raw = [0xFF; 1024];
    let mut chunk = 0;
    let end = (rqml as f32 / 1024.0).ceil() as i32;
    match client.readable().await {
        Ok(_) => {}
        Err(_) => {return;}
    }
    'outer: while chunk < end {
        match client.try_read(& mut raw){
            Ok(_) => {}
            Err(_) => {}
        }
        if raw[0] == 0xFF {
            break;
        }
        for pos in 0 .. 1024 {
            if raw[pos] == 0xFF {
                buf_r.extend_from_slice(&raw[0..pos]);
                break 'outer;
            } else if pos == 1023 {
                buf_r.extend_from_slice(&raw);
            }
        }
        raw.fill(0xFF);
        chunk += 1;
    }
    let buf = String::from_utf8_lossy(&buf_r);
    match client.writable().await {
        Ok(_) => {}
        Err(_) => {return;}
    }
    if buf.len() < 3 {
        client.try_write(b"HTTP/1.1 404 NOT FOUND\r\n\r\n").expect("");
        return;
    }
    if &buf[..3] == "GET" {
        let path = &buf[4 .. buf.find(" HTTP").unwrap()];
        println!("{}", path);
        if !get.contains_key(path) {
            client.try_write(b"HTTP/1.1 404 NOT FOUND\r\n\r\n").expect("");
            return;
        }
        client.try_write(get[path].0(&get[path].1).as_bytes()).unwrap();
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