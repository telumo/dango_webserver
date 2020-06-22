use regex::Regex;
extern crate regex;

use std::collections::HashMap;
use std::io::{BufReader, Error, Read, Write};
use std::net::SocketAddr;
use std::{env, fs, str};

use mio::net::{TcpListener, TcpStream};
use mio::{Event, Events, Poll, PollOpt, Ready, Token};

const SERVER: Token = Token(0);
const WEBROOT: &str = "/webroot";

struct WebServer {
    address: SocketAddr,
    connections: HashMap<usize, TcpStream>,
    next_connection_id: usize,
}

impl WebServer {
    fn new(addr: &str) -> Self {
        let address = addr.parse().unwrap();
        WebServer {
            address,
            connections: HashMap::new(),
            next_connection_id: 1,
        }
    }

    fn make_response(buffer: &[u8], nbytes: &usize) -> Result<Vec<u8>, Error> {}
}

impl WebServer {
    fn run(&mut self) -> Result<(), Error> {
        let server = TcpListener::bind(&self.address).expect("Failed to bind address");
        let poll = Poll::new().unwrap();

        //サーバーソケットの状態を監視対象に登録する。
        poll.register(&server, SERVER, Ready::readable(), PollOpt::edge())
            .unwrap();

        // 場所作ってるだけ
        let mut events = Events::with_capacity(1024);
        let mut response = Vec::new();

        loop {
            poll.poll(&mut events, None).unwrap();
            for event in &events {
                match event.token() {
                    SERVER => {
                        self.connection_handler(&server, &poll);
                    }
                    Token(conn_id) => self.http_handler(conn_id, event, &poll, &mut response),
                }
            }
        }
    }

    fn connection_handler(&mut self, server: &TcpListener, poll: &Poll) {}

    fn http_handler(&mut self, conn_id: usize, event: Event, poll: &Poll, response: &mut Vec<u8>) {}
}

fn main() {
    println!("Hello, world!");
}
