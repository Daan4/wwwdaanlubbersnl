use std::{env, fs};
use wwwdaanlubbersnl::webserver::*;

fn main() {
    let ip: String;
    let port = match env::var("PORT") {
        Ok(port) => {
            ip = "0.0.0.0".to_string();
            port
        }
        Err(_) => {
            println!("No PORT environment variable found, using default port 8080");
            ip = "127.0.0.1".to_string();
            "8080".to_string()
        }
    };

    let config = AppConfig::new(format!("{}:{}", ip, port).parse().unwrap(), 4, 5);
    let mut app = create_app(config);
    register_resources(&mut app);
    app.run(None);
}

fn register_resources(app: &mut App) {
    register_all_resources_in_folder_for_get(app, "/", "static/html");
    register_all_resources_in_folder_for_get(app, "/", "static/css");
    register_all_resources_in_folder_for_get(app, "/", "static/images");

    app.register_resource(Resource::new(
        RequestType::GET,
        "/maria".to_string(),
        ResourceType::REDIRECT,
        Box::new(|| {
            Ok(Response::new(
                StatusCode::PermanentRedirect,
                "https://www.mariagomez.art".to_string(),
            ))
        }),
    ));
}

fn register_all_resources_in_folder_for_get(app: &mut App, base_path: &str, folder: &str) {
    let files: fs::ReadDir = fs::read_dir(folder).unwrap();
    for file in files {
        let file = file.unwrap();
        let file_name = file.file_name().into_string().unwrap();
        let file_ext = match file_name.split('.').last() {
            Some(ext) => ext,
            None => "",
        };
        let resource_type = match file_ext {
            "html" => ResourceType::TEXT,
            "css" => ResourceType::TEXT,
            "js" => ResourceType::TEXT,
            _ => ResourceType::BINARY,
        };

        let path = format!("{}/{}", folder, file_name);
        let resource = Resource::new(
            RequestType::GET,
            format!("{}{}", base_path, file_name),
            resource_type,
            Box::new(move || Ok(Response::new(StatusCode::OK, path.clone()))),
        );
        app.register_resource(resource);
    }
}
