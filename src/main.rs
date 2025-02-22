use std::{
    env,
    fs,
    path::Path,
};
use wwwdaanlubbersnl::webserver::*;

fn main() {
    let config = AppConfig::new(
        format!("0.0.0.0:{}", env::var("PORT").unwrap())
            .parse()
            .unwrap(),
        4,
        5,
    );
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
        || {
            Ok(Response::new(
                StatusCode::PermanentRedirect,
                "https://www.mariagomez.art".to_string(),
            ))
        },
    ));
}

fn register_all_resources_in_folder_for_get(app: &mut App, base_path: &str, folder: &str) {
    let files: fs::ReadDir = fs::read_dir(folder).unwrap();
    for file in files {
        let file = file.unwrap();
        let file_name = file.file_name().into_string().unwrap();
        let file_ext = Path::new(file_name.as_str()).extension().unwrap().to_str().unwrap();
        let resource_type = match file_ext {
            "html" => ResourceType::TEXT,
            "css" => ResourceType::TEXT,
            "js" => ResourceType::TEXT,
            _ => ResourceType::BINARY,
        };

        let resource = Resource::new(
            RequestType::GET,
            format!("{}/{}", base_path, file_name),
            resource_type,
            move || Ok(Response::new(StatusCode::OK, format!("{}/{}", base_path, file_name))),
        );
        app.register_resource(resource);
    }
}
