use crate::concurrency::ThreadPool;
use core::fmt::{self, Display};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
};

#[derive(PartialEq)]
pub enum RequestType {
    GET,
    POST,
    PUT,
    DELETE,
}

pub enum StatusCode {
    OK,
    NotFound,
    InternalServerError,
}

impl Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let output;
        match *self {
            StatusCode::OK => output = "HTTP/1.1 200 OK",
            StatusCode::NotFound => output = "HTTP/1.1 404 NOT FOUND",
            StatusCode::InternalServerError => output = "HTTP/1.1 500 INTERNAL SERVER ERROR",
        }
        write!(f, "{}", output)
    }
}

pub enum ResourceType {
    HTML,
    IMAGE,
}

pub struct Resource {
    request_type: RequestType,
    path: &'static str,
    resource_type: ResourceType,
    handler: fn() -> Result<Response, String>,
}

impl Resource {
    pub fn new(
        request_type: RequestType,
        path: &'static str,
        resource_type: ResourceType,
        handler: fn() -> Result<Response, String>,
    ) -> Resource {
        Resource {
            request_type,
            path,
            resource_type,
            handler,
        }
    }

    pub fn handle(&self) -> Result<Response, String> {
        (self.handler)()
    }
}

pub struct Response {
    status_code: StatusCode,
    file: &'static str,
}

impl Response {
    pub fn new(status_code: StatusCode, file: &'static str) -> Response {
        Response { status_code, file }
    }
}

pub struct AppConfig {
    ip: &'static str,
    port: u16,
    num_threads: usize,
}

impl AppConfig {
    pub fn new(ip: &'static str, port: u16, num_threads: usize) -> AppConfig {
        AppConfig {
            ip,
            port,
            num_threads,
        }
    }
}

pub struct App {
    config: AppConfig,
    resources: Vec<Resource>,
    resource_404: Option<Resource>,
}

impl App {
    pub fn new(config: AppConfig) -> App {
        App {
            config,
            resources: vec![],
            resource_404: None,
        }
    }

    pub fn run(self) {
        let ip = self.config.ip;
        let port = self.config.port;
        let listener = TcpListener::bind(format!("{ip}:{port}")).unwrap();
        let pool = ThreadPool::new(self.config.num_threads);
        let app = Arc::new(self);

        for stream in listener.incoming() {
            let stream = stream.unwrap();
            let app_clone = Arc::clone(&app);

            pool.execute(move || app_clone.handle_request(stream));
        }
    }

    pub fn register_resource(&mut self, resource: Resource) {
        self.resources.push(resource);
    }

    pub fn register_resource_404(&mut self, resource: Resource) {
        self.resource_404 = Some(resource);
    }

    fn get_resource(&self, request_type: RequestType, path: &str) -> Option<&Resource> {
        self.resources
            .iter()
            .find(|resource| resource.request_type == request_type && resource.path == path)
    }

    fn handle_resource(&self, resource: &Resource, stream: &mut TcpStream) {
        let response = resource.handle().unwrap();
        let filename = response.file;
        let status = response.status_code;

        match resource.resource_type {
            ResourceType::HTML => self.handle_html(filename, status, stream),
            ResourceType::IMAGE => self.handle_image(filename, status, stream),
        }
    }

    fn handle_html(&self, filename: &str, status: StatusCode, stream: &mut TcpStream) {
        let content = fs::read_to_string(filename).unwrap();
        let length = content.len();
        let response = format!("{status}\r\nContent-Length: {length}\r\n\r\n{content}");

        print!("Response: {response}\n");
        stream.write_all(response.as_bytes()).unwrap();
    }

    fn handle_image(&self, filename: &str, status: StatusCode, stream: &mut TcpStream) {
        let content: &[u8] = &fs::read(filename).unwrap();
        let length = content.len();
        let response = format!("{status}\r\nContent-Length: {length}\r\n\r\n");
        print!("Response: {response}<snip>\n");
        stream
            .write_all(&[response.as_bytes(), content].concat())
            .unwrap();
    }

    fn handle_not_found(&self, stream: &mut TcpStream) {
        let resource = &self.resource_404;
        match resource {
            Some(resource) => self.handle_resource(&resource, stream),
            None => {
                let response = format!("{}\r\nContent-Length: 0\r\n\r\n", StatusCode::NotFound);
                print!("Response: {response}\n");
                stream.write_all(response.as_bytes()).unwrap();
            }
        }
    }

    fn handle_request(&self, mut stream: TcpStream) {
        let buf_reader = BufReader::new(&stream);
        let request_line = buf_reader.lines().next().unwrap().unwrap();

        print!("Request: {request_line}\n");

        let parts = request_line.split_whitespace().collect::<Vec<&str>>();

        if parts.len() < 2 {
            // Handle error case when request line is malformed
            return;
        }

        let request_type = match parts[0] {
            "GET" => RequestType::GET,
            "POST" => RequestType::POST,
            "PUT" => RequestType::PUT,
            "DELETE" => RequestType::DELETE,
            _ => RequestType::GET, // todo unsupported request
        };

        let path = parts[1];

        let resource = self.get_resource(request_type, path);
        match resource {
            Some(resource) => self.handle_resource(resource, &mut stream),
            None => self.handle_not_found(&mut stream),
        }
    }
}