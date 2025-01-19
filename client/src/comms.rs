use serde::Serialize;

#[derive(Serialize)]
#[serde(tag = "tag")]
#[serde(rename_all = "snake_case")]
pub enum Message {
    ClientInit {
        launch_directory: String,
        start_directory: String,
        start_query: String,
        recursive: bool,
        file_mode: String,
    },
    Result {
        query: String,
        key: String,
        selection: Vec<String>,
        code: i32,
    },
}
