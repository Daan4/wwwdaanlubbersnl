use wwwdaanlubbersnl::webserver::*;

fn main() {
    let config = AppConfig::new("127.0.0.1:7676".parse().unwrap(), 4, 5);
    let mut app = create_app(config);
    register_resources(&mut app);
    app.run(None);
}

// todo register resources from old website. Maybe make a function to auto register a folder of resources for get request?
fn register_resources(app: &mut App) {
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

    app.register_resource_500(Resource::new(
        RequestType::GET,
        "/500",
        ResourceType::HTML,
        || {
            Ok(Response::new(
                StatusCode::InternalServerError,
                "static/500.html",
            ))
        },
    ));

    app.register_resource(Resource::new(
        RequestType::GET,
        "/maria",
        ResourceType::REDIRECT,
        || {
            Ok(Response::new(
                StatusCode::PermanentRedirect,
                "https://www.mariagomez.art",
            ))
        },
    ));

    app.register_resource(Resource::new(
        RequestType::GET,
        "/static/bootstrap.css",
        ResourceType::HTML,
        || {
            Ok(Response::new(
                StatusCode::OK,
                "static/bootstrap.css",
            ))
        },
    ));

    app.register_resource(Resource::new(
        RequestType::GET,
        "/static/github_logo.png",
        ResourceType::IMAGE,
        || {
            Ok(Response::new(
                StatusCode::OK,
                "static/github_logo.png",
            ))
        },
    ));

    app.register_resource(Resource::new(
        RequestType::GET,
        "/static/lichess_logo.webp",
        ResourceType::IMAGE,
        || {
            Ok(Response::new(
                StatusCode::OK,
                "static/lichess_logo.webp",
            ))
        },
    ));

    app.register_resource(Resource::new(
        RequestType::GET,
        "/static/linkedin_logo.png",
        ResourceType::IMAGE,
        || {
            Ok(Response::new(
                StatusCode::OK,
                "static/linkedin_logo.png",
            ))
        },
    ));    

    app.register_resource(Resource::new(
        RequestType::GET,
        "/static/medium_logo.webp",
        ResourceType::IMAGE,
        || {
            Ok(Response::new(
                StatusCode::OK,
                "static/medium_logo.webp",
            ))
        },
    ));    

    app.register_resource(Resource::new(
        RequestType::GET,
        "/static/nano_logo.png",
        ResourceType::IMAGE,
        || {
            Ok(Response::new(
                StatusCode::OK,
                "static/nano_logo.png",
            ))
        },
    ));  

    app.register_resource(Resource::new(
        RequestType::GET,
        "/static/maria_logo.png",
        ResourceType::IMAGE,
        || {
            Ok(Response::new(
                StatusCode::OK,
                "static/maria_logo.png",
            ))
        },
    ));  
    
}
