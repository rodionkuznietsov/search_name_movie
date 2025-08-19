use dashmap::{DashMap, DashSet};
use tokio::{self, fs};
use serde::{ser, Deserialize};
use reqwest::{self, Client};
use std::{default::Default, fs::{File, OpenOptions}, io::Write, time::Duration};
use serde_json::{json, Value};

const API_BOT_TOKEN: &str = "8425753701:AAHca7QlAtCPgl6J_os_nLUyELRDlfKSD60";
const SERP_API_GOOGLE_LENS: &str = "aaf525af5fca13fd06b57a8e3f382d1b3f5f1ce6ed07621880cf91dbab44d393";

#[derive(Debug, Default, Deserialize)]
struct ApiResponse {
    #[serde(default)]
    result: Vec<ResultResponse>
}

#[derive(Debug, Default, Deserialize)]
struct ResultResponse {
    #[serde(default)]
    update_id: u64,
    #[serde(default)]
    message: Message
}

#[derive(Debug, Default, Deserialize)]
struct ApiResponse2 {
    #[serde(default)]
    result: ResultResponse2
}

#[derive(Debug, Default, Deserialize)]
struct ResultResponse2 {
    #[serde(default)]
    file_path: String
}

#[derive(Debug, Default, Deserialize)]
struct Message {
    #[serde(default)]
    chat: Chat,
    #[serde(default)]
    text: String,
    #[serde(default)]
    photo: Vec<PhotoResponse>
}

#[derive(Debug, Default, Deserialize)]
struct PhotoResponse {
    file_id: String,
    file_unique_id: String,
    file_size: u64,
    width: u64,
    height: u64
}

impl Message {
    fn get_command(&self) -> String {
        self.text.to_string()
    }
}

#[derive(Debug, Default, Deserialize)]
struct Chat {
    #[serde(default)]
    id: u64
}

impl Chat {
    fn get_id(&self) -> u64 {
        self.id
    }
}

#[derive(Debug, Default, Deserialize)]
struct GoogleLensApiResponse {
    #[serde(default)]
    visual_matches: Vec<VisualMatches>
}

#[derive(Debug, Default, Deserialize)]
struct VisualMatches {
    #[serde(default)]
    title: String
}

async fn send_message(chat_id: u64, text: String, reply_markup: bool, client: Client, keyboard: Option<Value>, divide_text: bool) {
    tokio::time::sleep(Duration::from_millis(50)).await;

    if divide_text {
        // We divide the text into parts
        let mut end = 20;
        let parts: Vec<&str> = text.split("%0A").collect();

        let mut jj = false;

        let mut text = String::new();
        for (i, str) in parts.iter().enumerate() {
            text.push_str(&format!("{}%0A", str));

            if i == end {
                jj = true;
                // println!("{text}");

                let mut url = format!("https://api.telegram.org/bot{}/sendMessage?parse_mode=HTML&chat_id={}&text={}", API_BOT_TOKEN, chat_id, text); 
        
                if reply_markup {
                    if let Some(ref keyboard) = keyboard {
                        url = format!("https://api.telegram.org/bot{}/sendMessage?chat_id={}&text={}&reply_markup={}", API_BOT_TOKEN, chat_id, text, keyboard); 
                    }
                }

                let response = client.get(url)
                    .send()
                    .await
                    .unwrap();

                println!("Status: {}", response.status());
                
                text.clear(); // Clear old text
                end += 10;
                // tokio::time::sleep(Duration::from_millis(300)).await;
            }
        }
    } else {
        let mut url = format!("https://api.telegram.org/bot{}/sendMessage?parse_mode=HTML&chat_id={}&text={}", API_BOT_TOKEN, chat_id, text); 

        if reply_markup {
            if let Some(ref keyboard) = keyboard {
                url = format!("https://api.telegram.org/bot{}/sendMessage?chat_id={}&text={}&reply_markup={}", API_BOT_TOKEN, chat_id, text, keyboard); 
            }
        }

        let response = client.get(url)
            .send()
            .await
            .unwrap();

        println!("Status: {}", response.status());
    }
}

async fn get_updates() {

    let offset = fs::read_to_string("./src/offset.txt").await.unwrap();
    let url = format!("https://api.telegram.org/bot{}/getUpdates?offset={}", API_BOT_TOKEN, offset);

    let client = reqwest::Client::new();
    let response = client.get(url)
        .send()
        .await.unwrap();
    let json: ApiResponse = response.json().await.unwrap();

    for result in json.result {
        let next_update_id = result.update_id + 1;

        let mut offset_file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open("./src/offset.txt")
            .unwrap();

        offset_file.write_all(next_update_id.to_string().as_bytes()).unwrap();

        let chat_id = result.message.chat.get_id();
        if chat_id > 0 {
            let command = result.message.get_command();

            if command == "/start" {
                let keyboard = json!(
                    {
                        "keyboard": [["Найти названия фильма/сериала"]],
                        "resize_keyboard": true,
                        "one_time_keyboard": true
                    }
                );

                let text = "Ещё раз, привет, что будем делать?".to_string();
                send_message(chat_id, text, true, client.clone(), Some(keyboard), false).await;
            }
            else if command == "Найти названия фильма/сериала" {
                let text = "Отправьте мне одно, желательно несколько фото, чтобы я смог по ним найти имя фильма/сериала.".to_string();
                send_message(chat_id, text, false, client.clone(), None, false).await;
            } 
            else {
                let mut min_width: u64 = 0;
                let mut file_photo_id = "".to_string();

                for photo_data in result.message.photo {
                    let photo_id = photo_data.file_id.clone();
                    let width = photo_data.width;
                    
                    if min_width < width {
                        min_width = width;
                        file_photo_id = photo_id;
                    }
                }

                if min_width > 0 {
                    println!("{}", file_photo_id);

                    let url = format!("https://api.telegram.org/bot{}/getFile?file_id={}", API_BOT_TOKEN, file_photo_id);

                    let response = client.get(url)
                        .send()
                        .await
                        .unwrap();

                    let json: ApiResponse2 = response.json().await.unwrap();
                    let photo_url = format!("https://api.telegram.org/file/bot{}/{}", API_BOT_TOKEN, json.result.file_path);
                    
                    let google_lens_url = format!("https://serpapi.com/search.json?engine=google_lens&url={}&api_key={}", photo_url, SERP_API_GOOGLE_LENS);
                    
                    let google_lens_response = client.get(google_lens_url)
                        .send()
                        .await
                        .unwrap();

                    let google_lens_json: GoogleLensApiResponse = google_lens_response.json().await.unwrap();
                    let mut text = String::new();

                    for (i, visual_matches) in google_lens_json.visual_matches.iter().enumerate() {
                        let title = visual_matches.title.clone();
                        text.push_str(&format!("{}: {}%0A%0A", i + 1, title));
                    }

                    text = format!("<b>Нашел {} совпадений</b> %0A%0A{}", google_lens_json.visual_matches.len(), text);
                    send_message(chat_id,  text, false, client, None, true).await;
                }

                return;
            }
        }
    }

}

#[tokio::main]
async fn main() {
    // get_updates().await;

    loop {
        get_updates().await;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
