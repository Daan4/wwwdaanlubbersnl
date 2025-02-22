use crate::concurrency::ThreadPool;
use core::fmt::{self, Display};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[derive(PartialEq, Debug)]
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
    read_timeout: u64,
}

impl AppConfig {
    pub fn new(addr: SocketAddr, num_threads: usize, read_timeout: u64) -> AppConfig {
        AppConfig {
            addr,
            num_threads,
            read_timeout,
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
                    Err(_) => {
                        self.handle_error(stream);
                        return;
                    }
                },
                None => {
                    self.handle_error(stream);
                    return;
                }
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

    fn handle_html(&self, path: &str, status: StatusCode, stream: &mut TcpStream) {
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
                        self.handle_not_found(stream);
                        return;
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

    fn handle_image(&self, path: &str, status: StatusCode, stream: &mut TcpStream) {
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
                        self.handle_not_found(stream);
                        return;
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

    fn handle_error(&self, stream: &mut TcpStream) {
        let resource = &self.resource_500;
        match resource {
            Some(resource) => self.handle_resource(&resource, stream),
            None => {
                let response = format!(
                    "{}\r\nContent-Length: 0\r\n\r\n",
                    StatusCode::InternalServerError
                );
                print!("Response: {response}\n");
                match stream.write_all(response.as_bytes()) {
                    Err(e) => print!("Failed to write to stream: {e:?}\n"),
                    _ => {}
                }
            }
        }
    }

    fn handle_request(&self, mut stream: TcpStream) {
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(
                self.config.read_timeout,
            )))
            .unwrap();
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
            print!("Malformed request\n");
            return;
        }

        let request_type = match parts[0] {
            "GET" => RequestType::GET,
            "POST" => RequestType::POST,
            "PUT" => RequestType::PUT,
            "DELETE" => RequestType::DELETE,
            _ => {
                print!("Unsupported request\n");
                return;
            }
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
        io::Read,
        net::{Ipv4Addr, SocketAddrV4},
        thread, time,
    };

    const TEST_ADDR: SocketAddr =
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 7676));
    const STARTUP_TIME: u64 = 100;

    fn send_request(request_type: RequestType, path: &str) -> String {
        let mut stream = TcpStream::connect(TEST_ADDR).unwrap();

        let request = format!("{request_type:?} {path} HTTP/1.1\r\n");
        stream.write_all(request.as_bytes()).unwrap();

        let mut buf_reader = BufReader::new(&stream);
        let mut str = String::new();
        buf_reader.read_to_string(&mut str).unwrap();
        str
    }

    #[test]
    fn app_request_404() {
        // Default 400 handler
        let config = AppConfig::new(TEST_ADDR, 4, 5);
        let app = create_app(config);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let thread = thread::spawn(move || {
            app.run(Some(stop_flag_clone));
        });
        thread::sleep(time::Duration::from_millis(STARTUP_TIME)); // Give the app time to start up

        let response = send_request(RequestType::GET, "/");
        assert_eq!(
            response,
            "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 0\r\n\r\n"
        );

        let response = send_request(RequestType::POST, "/nonexistent");
        assert_eq!(
            response,
            "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 0\r\n\r\n"
        );

        let response = send_request(RequestType::PUT, "/im/not/real");
        assert_eq!(
            response,
            "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 0\r\n\r\n"
        );

        stop_flag.store(true, Ordering::SeqCst);
        let response = send_request(RequestType::DELETE, "deletemeplease");
        assert_eq!(
            response,
            "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 0\r\n\r\n"
        );

        thread.join().unwrap();

        // Custom 404 handler
        let config = AppConfig::new(TEST_ADDR, 4, 5);
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
        thread::sleep(time::Duration::from_millis(STARTUP_TIME)); // Give the app time to start up

        let response = send_request(RequestType::GET, "/");
        assert_eq!(response, "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 54\r\n\r\n<!DOCTYPE html><html lang=\"en\"><body>404</body></html>");

        let response = send_request(RequestType::POST, "/nonexistent");
        assert_eq!(response, "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 54\r\n\r\n<!DOCTYPE html><html lang=\"en\"><body>404</body></html>");

        let response = send_request(RequestType::PUT, "/im/not/real");
        assert_eq!(response, "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 54\r\n\r\n<!DOCTYPE html><html lang=\"en\"><body>404</body></html>");

        stop_flag.store(true, Ordering::SeqCst);
        let response = send_request(RequestType::DELETE, "deletemeplease");
        assert_eq!(response, "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 54\r\n\r\n<!DOCTYPE html><html lang=\"en\"><body>404</body></html>");

        thread.join().unwrap();
    }

    #[test]
    fn app_request_invalid() {
        let config = AppConfig::new(TEST_ADDR, 4, 1);
        let app = create_app(config);
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let thread = thread::spawn(move || {
            app.run(Some(stop_flag_clone));
        });
        thread::sleep(time::Duration::from_millis(STARTUP_TIME)); // Give the app time to start up

        let mut stream = TcpStream::connect(TEST_ADDR).unwrap();
        let mut str = String::new();

        stream.write_all("".as_bytes()).unwrap();
        let mut buf_reader = BufReader::new(&stream);
        buf_reader.read_to_string(&mut str).unwrap();
        assert_eq!(str, "");

        let mut stream = TcpStream::connect(TEST_ADDR).unwrap();
        stream.write_all("\n".as_bytes()).unwrap();
        let mut buf_reader = BufReader::new(&stream);
        buf_reader.read_to_string(&mut str).unwrap();
        assert_eq!(str, "");

        let mut stream = TcpStream::connect(TEST_ADDR).unwrap();
        stream.write_all("request\n".as_bytes()).unwrap();
        let mut buf_reader = BufReader::new(&stream);
        buf_reader.read_to_string(&mut str).unwrap();
        assert_eq!(str, "");

        let mut stream = TcpStream::connect(TEST_ADDR).unwrap();
        stream.write_all("some text here\n".as_bytes()).unwrap();
        let mut buf_reader = BufReader::new(&stream);
        buf_reader.read_to_string(&mut str).unwrap();
        assert_eq!(str, "");

        let mut stream = TcpStream::connect(TEST_ADDR).unwrap();
        stream
            .write_all(format!("FOO / HTTP/1.1\r\n").as_bytes())
            .unwrap();
        let mut buf_reader = BufReader::new(&stream);
        buf_reader.read_to_string(&mut str).unwrap();
        assert_eq!(str, "");

        let mut stream = TcpStream::connect(TEST_ADDR).unwrap();
        stream
            .write_all(format!("GET / HTTP/1.1").as_bytes())
            .unwrap();
        let mut buf_reader = BufReader::new(&stream);
        buf_reader.read_to_string(&mut str).unwrap();
        assert_eq!(str, "");

        stop_flag.store(true, Ordering::SeqCst);
        let stream = TcpStream::connect(TEST_ADDR).unwrap();
        drop(stream);

        thread.join().unwrap();
    }

    #[test]
    fn app_request_500() {
        // Default 500 handler
        let config = AppConfig::new(TEST_ADDR, 4, 5);
        let mut app = create_app(config);
        app.register_resource(Resource::new(
            RequestType::GET,
            "/",
            ResourceType::HTML,
            || Err("Failed".to_string()),
        ));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let thread = thread::spawn(move || {
            app.run(Some(stop_flag_clone));
        });
        thread::sleep(time::Duration::from_millis(STARTUP_TIME)); // Give the app time to start up

        stop_flag.store(true, Ordering::SeqCst);
        let response = send_request(RequestType::GET, "/");
        assert_eq!(
            response,
            "HTTP/1.1 500 INTERNAL SERVER ERROR\r\nContent-Length: 0\r\n\r\n"
        );

        thread.join().unwrap();

        // Custom 500 handler
        let config = AppConfig::new(TEST_ADDR, 4, 5);
        let mut app = create_app(config);
        app.register_resource(Resource::new(
            RequestType::GET,
            "/",
            ResourceType::HTML,
            || Err("Failed".to_string()),
        ));
        app.register_resource_500(Resource::new(
            RequestType::GET,
            "/500",
            ResourceType::HTML,
            || {
                Ok(Response::new(
                    StatusCode::InternalServerError,
                    "static_test/500.html",
                ))
            },
        ));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let thread = thread::spawn(move || {
            app.run(Some(stop_flag_clone));
        });
        thread::sleep(time::Duration::from_millis(STARTUP_TIME)); // Give the app time to start up

        stop_flag.store(true, Ordering::SeqCst);
        let response = send_request(RequestType::GET, "/");
        assert_eq!(response, "HTTP/1.1 500 INTERNAL SERVER ERROR\r\nContent-Length: 54\r\n\r\n<!DOCTYPE html><html lang=\"en\"><body>500</body></html>");

        thread.join().unwrap();
    }

    #[test]
    fn app_request() {
        let config = AppConfig::new(TEST_ADDR, 4, 5);
        let mut app = create_app(config);
        app.register_resource(Resource::new(
            RequestType::GET,
            "/html",
            ResourceType::HTML,
            || Ok(Response::new(StatusCode::OK, "static_test/test.html")),
        ));
        app.register_resource(Resource::new(
            RequestType::POST,
            "/html",
            ResourceType::HTML,
            || Ok(Response::new(StatusCode::OK, "static_test/test.html")),
        ));
        app.register_resource(Resource::new(
            RequestType::PUT,
            "/html",
            ResourceType::HTML,
            || Ok(Response::new(StatusCode::OK, "static_test/test.html")),
        ));
        app.register_resource(Resource::new(
            RequestType::DELETE,
            "/html",
            ResourceType::HTML,
            || Ok(Response::new(StatusCode::OK, "static_test/test.html")),
        ));
        app.register_resource(Resource::new(
            RequestType::GET,
            "/image",
            ResourceType::IMAGE,
            || Ok(Response::new(StatusCode::OK, "static_test/test.jpg")),
        ));
        app.register_resource(Resource::new(
            RequestType::POST,
            "/image",
            ResourceType::IMAGE,
            || Ok(Response::new(StatusCode::OK, "static_test/test.jpg")),
        ));
        app.register_resource(Resource::new(
            RequestType::PUT,
            "/image",
            ResourceType::IMAGE,
            || Ok(Response::new(StatusCode::OK, "static_test/test.jpg")),
        ));
        app.register_resource(Resource::new(
            RequestType::DELETE,
            "/image",
            ResourceType::IMAGE,
            || Ok(Response::new(StatusCode::OK, "static_test/test.jpg")),
        ));
        app.register_resource(Resource::new(
            RequestType::GET,
            "/redirect",
            ResourceType::REDIRECT,
            || Ok(Response::new(StatusCode::OK, "static_test/redirect.html")),
        ));
        app.register_resource(Resource::new(
            RequestType::POST,
            "/redirect",
            ResourceType::REDIRECT,
            || Ok(Response::new(StatusCode::OK, "static_test/redirect.html")),
        ));
        app.register_resource(Resource::new(
            RequestType::PUT,
            "/redirect",
            ResourceType::REDIRECT,
            || Ok(Response::new(StatusCode::OK, "static_test/redirect.html")),
        ));
        app.register_resource(Resource::new(
            RequestType::DELETE,
            "/redirect",
            ResourceType::REDIRECT,
            || Ok(Response::new(StatusCode::OK, "static_test/redirect.html")),
        ));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_clone = stop_flag.clone();
        let thread = thread::spawn(move || {
            app.run(Some(stop_flag_clone));
        });
        thread::sleep(time::Duration::from_millis(STARTUP_TIME)); // Give the app time to start up

        let response = send_request(RequestType::GET, "/html");
        assert_eq!(response, "HTTP/1.1 200 OK\r\nContent-Length: 55\r\n\r\n<!DOCTYPE html><html lang=\"en\"><body>test</body></html>");
        let response = send_request(RequestType::POST, "/html");
        assert_eq!(response, "HTTP/1.1 200 OK\r\nContent-Length: 55\r\n\r\n<!DOCTYPE html><html lang=\"en\"><body>test</body></html>");
        let response = send_request(RequestType::PUT, "/html");
        assert_eq!(response, "HTTP/1.1 200 OK\r\nContent-Length: 55\r\n\r\n<!DOCTYPE html><html lang=\"en\"><body>test</body></html>");
        let response = send_request(RequestType::DELETE, "/html");
        assert_eq!(response, "HTTP/1.1 200 OK\r\nContent-Length: 55\r\n\r\n<!DOCTYPE html><html lang=\"en\"><body>test</body></html>");

        let response = send_request(RequestType::GET, "/image");
        assert_eq!(
            response,
            "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\n\\x01\\x02\\x03"
        );
        let response = send_request(RequestType::POST, "/image");
        assert_eq!(
            response,
            "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\n\\x01\\x02\\x03"
        );
        let response = send_request(RequestType::PUT, "/image");
        assert_eq!(
            response,
            "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\n\\x01\\x02\\x03"
        );
        let response = send_request(RequestType::DELETE, "/image");
        assert_eq!(
            response,
            "HTTP/1.1 200 OK\r\nContent-Length: 12\r\n\r\n\\x01\\x02\\x03"
        );

        let response = send_request(RequestType::GET, "/redirect");
        assert_eq!(
            response,
            "HTTP/1.1 200 OK\r\nLocation: static_test/redirect.html\r\nContent-Length: 0\r\n\r\n"
        );
        let response = send_request(RequestType::POST, "/redirect");
        assert_eq!(
            response,
            "HTTP/1.1 200 OK\r\nLocation: static_test/redirect.html\r\nContent-Length: 0\r\n\r\n"
        );
        let response = send_request(RequestType::PUT, "/redirect");
        assert_eq!(
            response,
            "HTTP/1.1 200 OK\r\nLocation: static_test/redirect.html\r\nContent-Length: 0\r\n\r\n"
        );
        stop_flag.store(true, Ordering::SeqCst);
        let response = send_request(RequestType::DELETE, "/redirect");
        assert_eq!(
            response,
            "HTTP/1.1 200 OK\r\nLocation: static_test/redirect.html\r\nContent-Length: 0\r\n\r\n"
        );

        thread.join().unwrap();
    }
}
