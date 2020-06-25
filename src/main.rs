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

    fn make_response(buffer: &[u8], nbytes: &usize) -> Result<Vec<u8>, Error> {
        let http_pattern = Regex::new(r"(.*) (.*) HTTP/1.([0-1])\r\n.*").unwrap();
        let captures = match http_pattern.captures(str::from_utf8(&buffer[..*nbytes]).unwrap()) {
            Some(cap) => cap,
            None => {
                // HTTP/1系でなければ、400を返す　ー＞ステータスライン（HTTPのバージョン、ステータスコード、リーズンフレーズ）
                let mut response = Vec::new();
                response.append(&mut "HTTP/1.0 400 Bad Request\r\n".to_string().into_bytes());
                response.append(&mut "Server: mio webserver\r\n".to_string().into_bytes());
                response.append(&mut "\r\n".to_string().into_bytes());
                return Ok(response);
            }
        };
        //　リクエストラインから抽出[メソッド、リクエストURI,　HTTPバージョン]
        let method = captures.get(1).unwrap().as_str();
        // リクエストパスから、ディレクトリパスに変換
        let path = &format!("{}{}{}", env::current_dir().unwrap().display(), WEBROOT, captures.get(2).unwrap().as_str());
        let _version = captures.get(3).unwrap().as_str();

        if method == "GET" {
            let file = match fs::File::open(path) {
                Ok(file) => file,
                // リクエストされたリソースがなければ404
                Err(_) => {
                    // パーミッションエラーなどもここに含まれるが手抜きしてnot foundにしている
                    let mut response = Vec::new();
                    response.append(&mut "HTTP/1.0 404 Not Found\r\n".to_string().into_bytes());
                    response.append(&mut "Server: mio webserver\r\n\r\n".to_string().into_bytes());
                    return Ok(response);
                }
            };

            // ファイルの読み込み
            let mut reader = BufReader::new(file);
            let mut buf = Vec::new();
            reader.read_to_end(&mut buf)?;

            // レスポンス
            let mut response = Vec::new();
            response.append(&mut "HTTP/1.0 200 OK\r\n".to_string().into_bytes());
            response.append(&mut "Server: mio webserver\r\n".to_string().into_bytes());
            response.append(&mut "\r\n".to_string().into_bytes());
            response.append(&mut buf);
            return Ok(response);
        }
        // サポートしていないHTTPメソッド
        let mut response = Vec::new();
        response.append(&mut "HTTP/1.0 501 Not Implemented\r\n".to_string().into_bytes());
        response.append(&mut "Server: mio webserver\r\n".to_string().into_bytes());
        response.append(&mut "\r\n".to_string().into_bytes());
        return Ok(response);
    }
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
            //現在のスレッドをブロックしてイベントを待つ。
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

    fn connection_handler(&mut self, server: &TcpListener, poll: &Poll) {
        let (stream, remote) = server.accept().expect("Failed to accept connection");
        println!("Connection from {}", &remote);

        let token = Token(self.next_connection_id);
        poll.register(&stream, token, Ready::readable(), PollOpt::edge()).unwrap();

        if let Some(_) = self.connections.insert(self.next_connection_id, stream){
            // HashMapは既存のキーで値が更新されると更新前の値を返す
            panic!("Failed to register connection");
        }
        self.next_connection_id += 1;
    }

    fn http_handler(&mut self, conn_id: usize, event: Event, poll: &Poll, response: &mut Vec<u8>) {
        if let Some(stream) = self.connections.get_mut(&conn_id) {
            if event.readiness().is_readable() {
                println!("conn_id: {}", conn_id);
                let mut buffer = [0u8; 512];
                let nbytes = stream.read(&mut buffer).expect("Failed toread");

                if nbytes != 0 {
                    *response = WebServer::make_response(&buffer, &nbytes).unwrap();
                    poll.reregister(stream, Token(conn_id), Ready::writable(), PollOpt::edge()).unwrap();
                } else {
                    self.connections.remove(&conn_id);
                }
            } else if event.readiness().is_writable() {
                stream.write(&response).expect("Failed to write");
                stream.flush().unwrap();
                self.connections.remove(&conn_id);
            }
        }
    }
}

fn main() {
    println!("Hello, world!");
    let args: Vec<String> = env::args().collect();

    if args.len() !=2 {
        eprintln!("Bad number of arguments");
        std::process::exit(1);
    }
    let mut server = WebServer::new(&args[1]);
    server.run().expect("Internal Server Error.");
}

// pollはI/Oを多重化するためのシステムコールの1つで、
// selectで問題だったファイルディスクリプタの数が無制限になっている。
// しかし，select同様監視対象のファイルディスクリプタを1つずつ状態確認するため、
// その計算量は*O(n)*となる。そのため、ファイルディスクリプタの数が増えると
// パフォーマンスが低下する。selectを利用するならpollを使う方が良い。
// seealso: https://blog.takanabe.tokyo/2015/03/%E3%83%8E%E3%83%B3%E3%83%96%E3%83%AD%E3%83%83%E3%82%AD%E3%83%B3%E3%82%B0i/o%E3%81%A8%E9%9D%9E%E5%90%8C%E6%9C%9Fi/o%E3%81%AE%E9%81%95%E3%81%84%E3%82%92%E7%90%86%E8%A7%A3%E3%81%99%E3%82%8B/
