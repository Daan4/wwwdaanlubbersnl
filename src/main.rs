use wwwdaanlubbersnl::webserver::{App, AppConfig, RequestType, Resource, ResourceType, Response, StatusCode};

fn main() {
    let config = AppConfig::new("127.0.0.1", 7878, 4);
    let app = create_app(config);
    app.run();
}

fn create_app(config: AppConfig) -> App {
    let mut app = App::new(config);

    app.register_resource(Resource::new(
        RequestType::GET,
        "/",
        ResourceType::HTML,
        || Ok(Response::new(StatusCode::OK, "static/index.html")),
    ));

    app.register_resource(Resource::new(
        RequestType::GET,
        "/favicon.ico",
        ResourceType::IMAGE,
        || Ok(Response::new(StatusCode::OK, "static/icon.ico")),
    ));

    app.register_resource_404(Resource::new(
        RequestType::GET,
        "/404",
        ResourceType::HTML,
        || Ok(Response::new(StatusCode::NotFound, "static/404.html")),
    ));

    app.register_resource(Resource::new(
        RequestType::GET,
        "/maria",
        ResourceType::REDIRECT,
        || Ok(Response::new(StatusCode::PermanentRedirect, "https://www.mariagomez.art")),
    ));

    app
}
