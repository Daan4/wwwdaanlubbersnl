use std::{
    fs,
    io::{prelude::*, BufReader},
    net::{TcpListener, TcpStream},  
    thread,
    time::Duration,
};

use wwwdaanlubbersnl::ThreadPool;

// Note: handle panics

fn main() {
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

    let pool = ThreadPool::new(4);

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        pool.execute(|| { handle_connection(stream) });
    }

    println!("Shutting down.");
}

fn handle_connection(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&stream);
    let request_line = buf_reader.lines().next().unwrap().unwrap();

    let (status, filename) = match &request_line[..] {
        "GET / HTTP/1.1" => ("HTTP/1.1 200 OK", "html/index.html"),
        "GET /sleep HTTP/1.1" => {
            thread::sleep(Duration::from_secs(5));
            ("HTTP/1.1 200 OK", "html/index.html")
        },
        _ => ("HTTP/1.1 404 NOT FOUND", "html/404.html"),
    };

    let content = fs::read_to_string(filename).unwrap();
    let length = content.len();
    let response = format!("{status}\r\nContent-Length: {length}\r\n\r\n{content}");

    stream.write_all(response.as_bytes()).unwrap();
}
