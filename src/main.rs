use dashmap::DashMap;
use heck::ToTitleCase;
use tokio::{self, fs};
use serde::{de, Deserialize};
use reqwest::{self, Client};
use std::{default::{self, Default}, fs::OpenOptions, io::Write, time::Duration};
use serde_json::{json, Value};
use pyo3::{types::{PyAnyMethods, PyModule}, PyResult, Python};
use std::ffi::CString;

const API_BOT_TOKEN: &str = "API_KEY";
const SERP_API_GOOGLE_LENS: &str = "API_KEY";

const SPACE: &str = "%0A";

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
    #[serde(default)]
    file_id: String,
    #[serde(default)]
    width: u64,
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

#[derive(Debug, Default, Deserialize)]
struct MovieResponse {
    #[serde(default)]
    knowledge_graph: KnowledgeGraph
}

#[derive(Debug, Default, Deserialize)]
struct KnowledgeGraph {
    #[serde(default)]
    title: String,
    #[serde(default)]
    movie: Option<String>
}

async fn send_message(chat_id: u64, text: String, reply_markup: bool, client: Client, keyboard: Option<Value>) {
    tokio::time::sleep(Duration::from_millis(50)).await;
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

async fn search_keyword(text: String) -> PyResult<String> {

    pyo3::prepare_freethreaded_python();

    let filename = CString::new("keywords_ai.py").unwrap();
    let module_name = CString::new("keywords_ai").unwrap();
    let path: CString = CString::new(include_str!("scripts/python/keywords_ai.py")).unwrap();
    
    let repeated_words: DashMap<String, u32> = DashMap::new();

    let result: PyResult<String> = tokio::task::spawn_blocking(move || {
        Python::with_gil(|py| {
            let p_script = PyModule::from_code(
                py, 
                path.as_c_str(), 
                filename.as_c_str(), 
                module_name.as_c_str()
            )?;

            // Список текстов
            let texts: Vec<&str> = text.split(&format!("{}", SPACE)).collect();

            let results: Vec<Vec<String>> = p_script
                .getattr("extract_keywords")?
                .call1((texts,))?
                .extract()?;

            let mut keyword = String::new();
            
            for results in results {
                for string in results {
                    let string = string.clone().to_lowercase();
                    let string = string.trim_end_matches("youtube",).to_string();
                    let string = string.trim_end_matches("wiki",).to_string();
                    let string = string.trim_end_matches("season",).to_string();
                    let string = string.trim_end_matches("imdb",).to_string();
                    let string = string.trim_end_matches("'s",).to_string();
                    let string = string.trim_end_matches(&['-', ' ']).to_string();

                    if let Some(mut value) = repeated_words.get_mut(&string) {
                        *value += 1; 
                    } else {
                        repeated_words.insert(string, 1);
                    }
                }
            }

            if let Some((key, value)) = repeated_words
                .iter()
                .max_by_key(|entry| *entry.value())
                .map(|entry| (entry.key().clone(), *entry.value()))
            {
                keyword = key.to_title_case();
            }

            Ok(keyword)
        })
    })
    .await.unwrap();

    result
}
    
async fn get_film_name(keyword: String, client: Client) -> Result<String, reqwest::Error> {
    let url = format!("https://serpapi.com/search.json?engine=google&q={} movie&api_key={}", keyword, SERP_API_GOOGLE_LENS);

    println!("{url}");

    let response = client.get(url)
        .send()
        .await?;

    println!("Status: {}", response.status());
        
    let movie_json: MovieResponse = response.json().await?;

    let mut movie_name = String::new();

    if movie_json.knowledge_graph.title == keyword {
        if let Some(movie) = movie_json.knowledge_graph.movie {
            movie_name = movie
        } else {
            movie_name = movie_json.knowledge_graph.title
        }
    } else {
        movie_name = movie_json.knowledge_graph.title
    }

    Ok(movie_name)
}

async fn get_updates() -> Result<(), reqwest::Error>{

    let offset = fs::read_to_string("./src/offset.txt").await.unwrap();
    let url = format!("https://api.telegram.org/bot{}/getUpdates?offset={}", API_BOT_TOKEN, offset);

    let client = reqwest::Client::new();
    let response = client.get(url)
        .send()
        .await?;
    let json: ApiResponse = response.json().await?;

    for result in json.result {
        let client = client.clone();
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
                send_message(chat_id, text, true, client.clone(), Some(keyboard)).await;
            }
            else if command == "Найти названия фильма/сериала" {
                let text = "Отправьте мне одно, желательно несколько фото, чтобы я смог по ним найти имя фильма/сериала.".to_string();
                send_message(chat_id, text, false, client.clone(), None).await;
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
                        .await?;

                    let json: ApiResponse2 = response.json().await.unwrap();
                    let photo_url = format!("https://api.telegram.org/file/bot{}/{}", API_BOT_TOKEN, json.result.file_path);
                    
                    let google_lens_url = format!("https://serpapi.com/search.json?engine=google_lens&url={}&api_key={}", photo_url, SERP_API_GOOGLE_LENS);
                    
                    let google_lens_response = client.get(google_lens_url)
                        .send()
                        .await?;

                    let google_lens_json: GoogleLensApiResponse = google_lens_response.json().await.unwrap();
                    let mut text = String::new();

                    for (i, visual_matches) in google_lens_json.visual_matches.iter().enumerate() {
                        let title = visual_matches.title.clone();
                        text.push_str(&format!("{}: {}{}{}", i + 1, title, SPACE, SPACE));
                    }

                    // text = format!("<b>Нашел {} совпадений</b> {}{}{}", google_lens_json.visual_matches.len(), SPACE, SPACE, text);
                    let keyword = search_keyword(text).await.expect("Не удалось получить ключивое слова");
                    let movie_name = get_film_name(keyword, client.clone()).await.expect("Не удалось получить имя фильма");

                    send_message(chat_id,  movie_name, false, client.clone(), None).await;
                }
            }
        }
    }

    Ok(())

}

#[tokio::main]
async fn main() {
    // get_updates().await;

    loop {
        get_updates().await.expect("Ошибка при получении новых обновлениях");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
