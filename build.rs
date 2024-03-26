use std::collections::HashMap;
use toml::Table;

fn main() {
    // collect all words and compile them into a Table
    let words = std::fs::read_dir("./res/sona-0.2.3/words/metadata/")
        .unwrap()
        .filter_map(|dir_entry| dir_entry.ok())
        .map(|dir_entry| dir_entry.path())
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .filter_map(|data| data.parse::<Table>().ok())
        .map(|table| (table["id"].to_owned().to_string().replace("\"", ""), table))
        .collect::<HashMap<String, Table>>();

    // convert Table to toml
    let words_toml = match toml::to_string(&words) {
        Ok(text) => text,
        Err(_) => {
            panic!("failed to convert to toml");
        }
    };

    /* let path = "res/words.toml";
    if std::fs::write(path, words_toml).is_err() {
        panic!("failed to save file {path}");
    } */

    // compress file with bzip2
    let compressor = bzip2::read::BzEncoder::new(words_toml.as_bytes(), bzip2::Compression::best());
    let words_toml_bz2: Vec<u8> = std::io::Read::bytes(compressor)
        .map(|x| x.unwrap()) // not sure why this is a result
        .collect();

    let path = "res/words.toml.bz2";
    if std::fs::write(path, words_toml_bz2).is_err() {
        panic!("failed to save file {path}");
    }
}
