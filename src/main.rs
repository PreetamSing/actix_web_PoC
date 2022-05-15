use actix_web::{
  get, http, post, web, App, HttpResponse, HttpServer, Responder, ResponseError,
};
// use derive_more::Display;
use actix_hello::env;
use config::Config;
use mongodb::{options::ClientOptions as MongoOptions, Client as MongoClient};
use reqwest::{header, Client};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

struct SharedState {
  value: Mutex<Vec<usize>>,
}

#[derive(Debug)]
struct StatusCode(http::StatusCode);

impl Serialize for StatusCode {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_u16(self.0.as_u16())
  }
}

// Structure of Request
#[derive(Deserialize, Serialize)]
struct GetRepoTags {
  username: String,
}

#[derive(Debug, Serialize)]
#[allow(non_snake_case)]
struct GetRepoTagsError {
  statusCode: StatusCode,
  message: String,
}

impl fmt::Display for GetRepoTagsError {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let err_json = serde_json::to_string(&self).unwrap();
    write!(f, "{}", err_json)
  }
}

impl ResponseError for GetRepoTagsError {
  fn error_response(&self) -> HttpResponse {
    HttpResponse::build(self.status_code())
      .insert_header(("Content-Type", "application/json"))
      .body(format!("{}", self))
  }
  fn status_code(&self) -> http::StatusCode {
    self.statusCode.0
  }
}

#[derive(Deserialize)]
struct RepoTagPath {
  repo: String,
}

#[derive(Deserialize, Serialize)]
struct Commit {
  sha: String,
  url: Option<String>,
  // This field is never provided by Github, so in the response
  // it is filled in with the value of null.
  absent: Option<bool>,
}

#[derive(Deserialize, Serialize)]
struct RepoTagResponse {
  name: String,
  commit: Option<Commit>,
  zipball_url: String,
  tarball_url: String,
  node_id: String,
}

#[get("/")]
async fn hello(data: web::Data<SharedState>) -> impl Responder {
  let mut value = data.value.lock().unwrap();
  // Thread goes to sleep while holding the lock, causing other
  // requests to same endpoint wait even longer. E.g.
  // req_1: waits 5s, req_2: waits 10s, and so on.
  thread::sleep(Duration::from_secs(5));
  *value = vec![value[0] + 1];
  let res = format!("{}", value[0]);
  HttpResponse::Ok().body(res)
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
  HttpResponse::Ok().body(req_body)
}

#[post("/{repo}/tags")]
async fn repo_tags(
  req_body: web::Json<GetRepoTags>,
  path_params: web::Path<RepoTagPath>,
) -> actix_web::Result<web::Json<Vec<RepoTagResponse>>, GetRepoTagsError> {
  let username = &req_body.username;
  let repo_name = path_params.into_inner().repo;
  let client = Client::new();
  let result = client
    .get(format!(
      "https://api.github.com/repos/{}/{}/tags",
      username, repo_name
    ))
    .header(header::USER_AGENT, "request")
    .send()
    .await;

  // If request fails with an error.
  if let Err(error) = result {
    eprintln!("{:#?}", error);
    return actix_web::Result::Err(GetRepoTagsError {
      statusCode: StatusCode(http::StatusCode::from_u16(500).unwrap()),
      message: "Something went wrong! Try again later.".to_owned(),
      // message: error.without_url().to_string(),
    });
  }

  // Shadowing
  let result = result.unwrap();

  // If api call was successful, but response wasn't as expected.
  if result.status() == 404 {
    return actix_web::Result::Err(GetRepoTagsError {
      statusCode: StatusCode(http::StatusCode::from_u16(404).unwrap()),
      message: "Not Found".to_owned(),
    });
  }

  let res = result.json::<Vec<RepoTagResponse>>().await;
  let res = res.map_err(|err| {
    eprintln!("{:#?}", err);
    return GetRepoTagsError {
      statusCode: StatusCode(http::StatusCode::from_u16(500).unwrap()),
      message: "Internal Server Error.".to_owned(),
    };
  })?;

  // If we've successfully received all the repo tags.
  Ok(web::Json(res))
}

async fn manual_hello() -> impl Responder {
  HttpResponse::Ok().body("Hey there!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  // Load environment config
  let settings = Config::builder()
    // Add in `./Settings.toml`
    .add_source(config::File::with_name("Settings"))
    // Add in settings from the environment (with a prefix of APP)
    // Eg.. `APP_DEBUG=1 ./target/app` would set the `debug` key
    .add_source(config::Environment::with_prefix("APP"))
    .build()
    .unwrap()
    .try_deserialize::<env::EnvConfig>()
    .unwrap();

  // Create mongo connection options.
  let mut mongo_options = MongoOptions::parse(settings.db_string)
    .await
    .map_err(|err| {
      eprintln!(
        "Failed to parse MongoDB connection string with following error:"
      );
      eprintln!("{:#?}", err);
      std::process::exit(1);
    })
    .unwrap();
  // Manually set an option.
  mongo_options.app_name = Some("My App".to_string());
  // Get a handle to the deployment.
  let mongo_client = MongoClient::with_options(mongo_options)
    .map_err(|err| {
      eprintln!("Failed to connect MongoDB with following error:");
      eprintln!("{:#?}", err);
      std::process::exit(1);
    })
    .unwrap();
  // List the names of the databases in that deployment.
  for db_name in mongo_client.list_database_names(None, None).await.unwrap() {
    println!("{}", db_name);
  }

  // Create shared state for the app, make sure to create it here with
  // web::Data type, so that the same data is shared across all worker
  // threads.
  let shared_state = web::Data::new(SharedState {
    value: Mutex::new(vec![0]),
  });
  HttpServer::new(move || {
    App::new().app_data(shared_state.clone()).service(
      web::scope("/api/v1")
        .service(hello)
        .service(echo)
        .service(repo_tags)
        .route("/hey", web::get().to(manual_hello)),
    )
  })
  .keep_alive(Duration::from_secs(60 * 60))
  .bind(("localhost", settings.port))
  .and_then(|app| {
    println!("Server running on {}!", &settings.port);
    Ok(app)
  })?
  .run()
  .await
}
