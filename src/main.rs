use reqwest::Client;
use once_cell::sync::OnceCell;
use serde::Deserialize;

const BASE_URL: &str = "https://apiv2.twitcasting.tv";

fn build_client(token: &str) -> Client {
    use reqwest::header::HeaderMap;
    let mut h = HeaderMap::new();
    h.insert("X-Api-Version", "2.0".parse().unwrap());
    h.insert(
        "Authorization",
        format!("Bearer {}", token).parse().unwrap(),
    );

    Client::builder().default_headers(h).build().unwrap()
}

#[derive(Deserialize, Debug)]
struct TwitcastError {
    code: u32,
    message: String,
    details: Option<serde_json::Value>
}

async fn get_movie_id(c: &Client, user: &str) -> Option<String> {
    let json: serde_json::Value = c
        .get(&format!("{}/users/{}/current_live", BASE_URL, user))
        .send()
        .await
        .expect("Failed to send a request while getting movie id")
        .json()
        .await
        .expect("Got a non-json response while getting movie id");
    if let Some(movie) = json.get("movie") {
        Some(movie["id"].as_str().unwrap().to_owned())
    } else if let Some(error) = json.get("error") {
        if let Ok(i) = serde_json::from_value::<TwitcastError>(error.clone()) {
            if i.code == 404 {
                None
            } else {
                panic!("Unexpected error : {:?}", i);
            }
        } else {
            panic!("Got a corrupted json while getting movie id : {}", json);
        }
    } else {
        panic!("Got a corrupted json while getting movie id : {}", json);
    }
}

async fn get_comments(
    c: &Client,
    movie_id: String,
    last_id: Option<String>,
) -> (Option<Vec<String>>, Option<String>) {
    let spec_slice = match &last_id {
        Some(i) => format!("?slice_id={}", i),
        None => String::new(),
    };
    let json: serde_json::Value = c
        .get(&format!(
            "{}/movies/{}/comments{}",
            BASE_URL, movie_id, spec_slice
        ))
        .send()
        .await
        .expect("Failed to send a request while getting comments")
        .json()
        .await
        .expect("Got a non-json response while getting comments");
    if let Some(comments) = json.get("comments") {
        let comments = comments.as_array().unwrap();
        (
            Some(
                comments
                    .iter()
                    .rev()
                    .map(|c| c["message"].as_str().unwrap().to_owned())
                    .collect(),
            ),
            match comments.first() {
                Some(i) => Some(i["id"].as_str().unwrap().to_owned()),
                None => last_id,
            },
        )
    } else if let Some(error) = json.get("error") {
        if let Ok(i) = serde_json::from_value::<TwitcastError>(error.clone()) {
            if i.code == 404 {
                (None, last_id)
            } else {
                panic!("Unexpected error : {:?}", i);
            }
        } else {
            panic!("Got a corrupted json while getting comments : {}", json);
        }
    } else {
        panic!("Got a corrupted json while getting comments : {}", json);
    }
}

use tokio::sync::mpsc;
use std::sync::Arc;

struct DataBundle {
    token: OnceCell<String>,
    client_id: String,
    client_secret: String,
    stopper: mpsc::Sender<()>
}

#[derive(Deserialize)]
struct CodeParam {
    code: String,
}

async fn callback_handler(q: CodeParam, data: Arc<DataBundle>) -> Result<String, warp::reject::Rejection> {
    let code: String = q.code.clone();
    let c = Client::new();
    let params = [
        ("code", &*code),
        ("grant_type", "authorization_code"),
        ("client_id", &*data.client_id),
        ("client_secret", &*data.client_secret),
        ("redirect_uri", "http://localhost:8000/"),
    ];
    let json = c
        .post(&format!("{}/oauth2/access_token", BASE_URL))
        .form(&params)
        .send()
        .await
        .expect("OAuth2 failed")
        .json::<serde_json::Value>()
        .await
        .expect("Got non-json response");
    if let Some(i) = json.get("access_token") {
        data.token.set(i.as_str().unwrap().to_owned()).unwrap();
    } else {
        panic!("Unexpected response : {}", json);
    }

    let mut stopper = data.stopper.clone();
    stopper.send(()).await.unwrap();

    Ok(
        "Thank you, the token has successfully retrieved.\nYou can now close the browser."
            .to_owned(),
    )
}

async fn get_token(client_id: String, client_secret: String) -> String {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);

    let bundle: Arc<DataBundle> = Arc::new(DataBundle {
        token: OnceCell::new(),
        client_id,
        client_secret,
        stopper: tx,
    });

    {
        use warp::Filter;
        let bundle2 = bundle.clone();
        let bundle_handle = warp::any().map(move || bundle2.clone());

        let token = warp::path::end()
            .and(warp::query::query::<CodeParam>())
            .and(bundle_handle)
            .and_then(callback_handler);

        let (_, svr) =
            warp::serve(token).bind_with_graceful_shutdown(([127, 0, 0, 1], 8000), async move {
                rx.recv().await;
            });

        svr.await;
    }

    bundle.token.get().unwrap().clone()
}

#[tokio::main]
async fn main() {
    let client_id =
        {
            let mut s = std::fs::read_to_string("client_id.txt").unwrap();
            s.pop();
            s
        };

    let client_secret =
        {
            let mut s = std::fs::read_to_string("client_secret.txt").unwrap();
            s.pop();
            s
        }
        ;

    eprintln!("Web browser will open to log you in, follow the instructions there");
    open::that(format!(
        "{}/oauth2/authorize?client_id={}&response_type=code",
        BASE_URL,
        client_id
    ))
    .unwrap();

    let token = get_token(client_id, client_secret).await;
    eprintln!("retrieved access token");

    let client = build_client(&token);
    let mut last_id = None;
    loop {
        eprintln!("retrive begin");
        let id = get_movie_id(&client, "equall2").await;
        if let Some(id) = id {
            let (comments, new_id) = get_comments(&client, id, last_id.clone()).await;
            last_id = new_id;

            if let Some(cs) = comments {
                eprintln!("retrieved {} comments", cs.len());
                for c in cs {
                    println!("{}", c);
                }
            } else {
                eprintln!("!! cannot retrieve comments");
            }
        } else {
            eprintln!("!! cannot retrieve movie id");
        }

        std::thread::sleep(std::time::Duration::from_secs(10));
    }
}
