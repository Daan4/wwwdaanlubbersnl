use crate::concurrency::ThreadPool;
use core::fmt::{self, Display};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream, SocketAddr},
    sync::{Arc, atomic::{AtomicBool, Ordering}},
};

#[derive(PartialEq)]
#[derive(Debug)]
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
    PermanentRedirect,
}

impl Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let output;
        match *self {
            StatusCode::OK => output = "HTTP/1.1 200 OK",
            StatusCode::NotFound => output = "HTTP/1.1 404 NOT FOUND",
            StatusCode::InternalServerError => output = "HTTP/1.1 500 INTERNAL SERVER ERROR",
            StatusCode::PermanentRedirect => output = "HTTP/1.1 301 PERMANENT REDIRECT",
        }
        write!(f, "{}", output)
    }
}

pub enum ResourceType {
    HTML,
    IMAGE,
    REDIRECT,
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
    path: &'static str,
}

impl Response {
    pub fn new(status_code: StatusCode, path: &'static str) -> Response {
        Response { status_code, path }
    }
}

pub struct AppConfig {
    addr: SocketAddr,
    num_threads: usize,
}

impl AppConfig {
    pub fn new(addr: SocketAddr, num_threads: usize) -> AppConfig {
        AppConfig {
            addr,
            num_threads,
        }
    }
}

pub struct App {
    config: AppConfig,
    resources: Vec<Resource>,
    resource_404: Option<Resource>,
    resource_500: Option<Resource>,
}

impl App {
    /// If the stop flag is set, the server will shut down after processing the next request.
    /// Implemented for testing purposes.
    pub fn new(config: AppConfig) -> App {
        App {
            config,
            resources: vec![],
            resource_404: None,
            resource_500: None,
        }
    }

    pub fn run(self, stop_flag: Option<Arc<AtomicBool>>) {
        let addr = self.config.addr;
        let listener = match TcpListener::bind(self.config.addr) {
            Ok(listener) => listener,
            Err(e) => panic!("Failed to bind to {addr}: {e:?}\n"),
        };
        let pool = ThreadPool::new(self.config.num_threads);
        let app = Arc::new(self);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let app_clone = Arc::clone(&app);

                    pool.execute(move || app_clone.handle_request(stream));
                }
                Err(e) => {
                    print!("Connection Failed: {e:?}")
                }
            }

            if let Some(stop_flag) = &stop_flag {
                if stop_flag.load(Ordering::SeqCst) {
                    break;
                }
            }
        }
    }

    pub fn register_resource(&mut self, resource: Resource) {
        self.resources.push(resource);
    }

    pub fn register_resource_404(&mut self, resource: Resource) {
        self.resource_404 = Some(resource);
    }

    pub fn register_resource_500(&mut self, resource: Resource) {
        self.resource_500 = Some(resource);
    }

    fn get_resource(&self, request_type: RequestType, path: &str) -> Option<&Resource> {
        self.resources
            .iter()
            .find(|resource| resource.request_type == request_type && resource.path == path)
    }

    fn handle_resource(&self, resource: &Resource, stream: &mut TcpStream) {
        let response = match resource.handle() {
            Ok(response) => response,
            Err(_) => match &self.resource_500 {
                Some(resource) => match resource.handle() {
                    Ok(response) => response,
                    Err(_) => Response::new(StatusCode::InternalServerError, ""),
                },
                None => Response::new(StatusCode::InternalServerError, ""),
            },
        };
        let path = response.path;
        let status = response.status_code;

        match resource.resource_type {
            ResourceType::HTML => self.handle_html(path, status, stream),
            ResourceType::IMAGE => self.handle_image(path, status, stream),
            ResourceType::REDIRECT => self.handle_redirect(path, status, stream),
        }
    }

    fn handle_html(&self, path: &str, mut status: StatusCode, stream: &mut TcpStream) {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => {
                let resource = &self.resource_404;
                match resource {
                    Some(resource) => {
                        self.handle_resource(&resource, stream);
                        return;
                    }
                    None => {
                        status = StatusCode::NotFound;
                        "".to_string()
                    }
                }
            }
        };
        let length = content.len();
        let response = format!("{status}\r\nContent-Length: {length}\r\n\r\n{content}");

        print!("Response: {response}\n");
        match stream.write_all(response.as_bytes()) {
            Err(e) => print!("Failed to write to stream: {e:?}\n"),
            _ => {}
        }
    }

    fn handle_image(&self, path: &str, mut status: StatusCode, stream: &mut TcpStream) {
        let content = match fs::read(path) {
            Ok(content) => content,
            Err(_) => {
                let resource = &self.resource_404;
                match resource {
                    Some(resource) => {
                        self.handle_resource(&resource, stream);
                        return;
                    }
                    None => {
                        status = StatusCode::NotFound;
                        vec![]
                    }
                }
            }
        };

        let length = content.len();
        let response = format!("{status}\r\nContent-Length: {length}\r\n\r\n");

        print!("Response: {response}<snip>\n");
        match stream.write_all(&[response.as_bytes(), &content].concat()) {
            Err(e) => print!("Failed to write to stream: {e:?}\n"),
            _ => {}
        }
    }

    fn handle_redirect(&self, path: &str, status: StatusCode, stream: &mut TcpStream) {
        let response = format!("{status}\r\nLocation: {path}\r\nContent-Length: 0\r\n\r\n");

        print!("Response: {response}\n");
        match stream.write_all(response.as_bytes()) {
            Err(e) => print!("Failed to write to stream: {e:?}\n"),
            _ => {}
        }
    }

    fn handle_not_found(&self, stream: &mut TcpStream) {
        let resource = &self.resource_404;
        match resource {
            Some(resource) => self.handle_resource(&resource, stream),
            None => {
                let response = format!("{}\r\nContent-Length: 0\r\n\r\n", StatusCode::NotFound);
                print!("Response: {response}\n");
                match stream.write_all(response.as_bytes()) {
                    Err(e) => print!("Failed to write to stream: {e:?}\n"),
                    _ => {}
                }
            }
        }
    }

    fn handle_request(&self, mut stream: TcpStream) {
        let buf_reader = BufReader::new(&stream);
        let request_line = match buf_reader.lines().next() {
            Some(line) => match line {
                Ok(line) => line,
                Err(e) => {
                    print!("Failed to read request line: {e:?}\n");
                    return;
                }
            },
            None => {
                print!("Empty request\n");
                return;
            }
        };

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

pub fn create_app(config: AppConfig) -> App {
    App::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        thread, 
        time,
        net::{SocketAddrV4, Ipv4Addr},
        io::Read,
    };

    const TEST_ADDR: SocketAddr =  SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 7676));
    const STARTUP_TIME: u64 = 1;

    fn send_request(request_type: RequestType, path: &str) -> String{
        let mut stream = TcpStream::connect(TEST_ADDR).unwrap();

        let request = format!("{request_type:?} {path} HTTP/1.1\r\n");
        stream.write_all(request.as_bytes()).unwrap();
        
        let mut buf_reader = BufReader::new(&stream);
        let mut str = String::new();
        buf_reader.read_to_string(&mut str).unwrap();
        str
    }

    #[test]
    fn app_default_404() {
        let config = AppConfig::new(TEST_ADDR, 4);
        let app = create_app(config);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let thread = thread::spawn(move || {
            app.run(Some(stop_flag_clone));
        });
        thread::sleep(time::Duration::from_secs(STARTUP_TIME)); // Give the app time to start up

        stop_flag.store(true, Ordering::SeqCst);
        let response = send_request(RequestType::GET, "/");
        assert_eq!(response, "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 0\r\n\r\n");
        thread.join().unwrap();
    }

    #[test]
    fn app_custom_404() {
        let config = AppConfig::new(TEST_ADDR, 4);
        let mut app = create_app(config);
        app.register_resource_404(Resource::new(
            RequestType::GET,
            "/404",
            ResourceType::HTML,
            || Ok(Response::new(StatusCode::NotFound, "static_test/404.html")),
        ));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let thread = thread::spawn(move || {
            app.run(Some(stop_flag_clone));
        });
        thread::sleep(time::Duration::from_secs(STARTUP_TIME)); // Give the app time to start up

        stop_flag.store(true, Ordering::SeqCst);
        let response = send_request(RequestType::GET, "/");
        assert_eq!(response, "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 54\r\n\r\n<!DOCTYPE html><html lang=\"en\"><body>404</body></html>");
        thread.join().unwrap();
    }

    #[test]
    fn app_invalid_requests() {
        let config = AppConfig::new(TEST_ADDR, 4);
        let app = create_app(config);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let thread = thread::spawn(move || {
            app.run(Some(stop_flag_clone));
        });
        thread::sleep(time::Duration::from_secs(STARTUP_TIME)); // Give the app time to start up

        
        let mut stream = TcpStream::connect(TEST_ADDR).unwrap();
        let mut str = String::new();

        stream.write_all("\n".as_bytes()).unwrap();     
        let mut buf_reader = BufReader::new(&stream); 
        buf_reader.read_to_string(&mut str).unwrap();
        assert_eq!(str, "");

        stop_flag.store(true, Ordering::SeqCst);
        let mut stream = TcpStream::connect(TEST_ADDR).unwrap();
        stream.write_all("asdfasdf\n".as_bytes()).unwrap();     
        let mut buf_reader = BufReader::new(&stream); 
        buf_reader.read_to_string(&mut str).unwrap();
        assert_eq!(str, "");

        thread.join().unwrap();
    }
}