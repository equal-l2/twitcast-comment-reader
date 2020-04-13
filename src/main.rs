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

fn get_movie_id(c: &Client) -> String {
    let resp = c
        .get(&format!("{}/users/equall2/current_live", BASE_URL))
        .send()
        .unwrap();
    if !resp.status().is_success() {
        panic!("Cannot retrieve movie id, maybe because the user is not streaming now: {}", resp.text().unwrap());
    }
    let json: serde_json::Value = resp.json().unwrap();
    json["movie"]["id"].as_str().unwrap().to_owned()
}

fn get_comments(
    c: &Client,
    movie_id: String,
    last_id: Option<String>,
) -> (Vec<String>, Option<String>) {
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
            comments.into_iter().rev().map(|c| c["message"].as_str().unwrap().to_owned()).collect(),
            match comments.first() {
                Some(i) => Some(i["id"].as_str().unwrap().to_owned()),
                None => last_id,
            },
        )
    } else {
        panic!("Cannot retrieve comments: {}", resp_com.text().unwrap());
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
        let id = get_movie_id(&client);
        let (comments, new_id) = get_comments(&client, id, last_id.clone());
        last_id = new_id;

        for c in comments {
            println!("{}", c);
        }

        eprintln!("retrive end");
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}
