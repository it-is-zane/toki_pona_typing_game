use std::collections::HashMap;

use toml::Table;

fn main() {
    let words = std::fs::read_dir("./res/sona-0.2.3/words/metadata/")
        .unwrap()
        .filter_map(|dir_entry| dir_entry.ok())
        .map(|dir_entry| dir_entry.path())
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .filter_map(|data| data.parse::<Table>().ok())
        .map(|table| (table["id"].to_owned().to_string().replace("\"", ""), table))
        .collect::<HashMap<String, Table>>();

    let words_toml = match toml::to_string(&words) {
        Ok(text) => text,
        Err(_) => {
            panic!("failed to convert to toml");
        }
    };

    if std::fs::write("res/words.toml", words_toml).is_err() {
        panic!("failed to save file");
    }
}
