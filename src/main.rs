use wwwdaanlubbersnl::{App, AppConfig, RequestType, StatusCode, Resource, Response};

fn main() {
    let config = AppConfig::new("127.0.0.1", 7878, 4);
    let app = create_app(config);
    app.run();
}

fn create_app(config: AppConfig) -> App {
    let mut app = App::new(config);

    app.register_resource(Resource::new(RequestType::GET, "/", || {
        Ok(Response::new(StatusCode::OK, "html/index.html"))      
    }));

    app
}
