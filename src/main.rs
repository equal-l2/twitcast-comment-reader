use reqwest::blocking::Client;

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

fn get_movie_id(c: &Client, user: &str) -> Option<String> {
    let resp = c
        .get(&format!("{}/users/{}/current_live", BASE_URL, user))
        .send()
        .unwrap();
    if resp.status().is_success() {
        let json: serde_json::Value = resp.json().unwrap();
        Some(json["movie"]["id"].as_str().unwrap().to_owned())
    } else {
        None
    }
}

fn get_comments(
    c: &Client,
    movie_id: String,
    last_id: Option<String>,
) -> (Option<Vec<String>>, Option<String>) {
    let spec_slice = match &last_id {
        Some(i) => format!("?slice_id={}", i),
        None => String::new(),
    };
    let resp_com = c
        .get(&format!(
            "{}/movies/{}/comments{}",
            BASE_URL,
            movie_id,
            spec_slice
        ))
        .send()
        .unwrap();
    if resp_com.status().is_success() {
        let json = resp_com.json::<serde_json::Value>().unwrap();
        let comments = json["comments"].as_array().unwrap();
        (
            Some(comments.into_iter().rev().map(|c| c["message"].as_str().unwrap().to_owned()).collect()),
            match comments.first() {
                Some(i) => Some(i["id"].as_str().unwrap().to_owned()),
                None => last_id,
            },
        )
    } else {
        (None, last_id)
    }
}

fn main() {
    let token = {
        let mut s = std::fs::read_to_string("token.txt").unwrap();
        s.pop();
        s
    };
    let client = build_client(&token);
    let mut last_id = None;
    loop {
        eprintln!("retrive begin");
        let id = get_movie_id(&client, "equall2");
        if let Some(id) = id {
            let (comments, new_id) = get_comments(&client, id, last_id.clone());
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
