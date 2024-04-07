use std::collections::HashMap;
use toml::Table;

fn main() {
    // get extra information from commentary.toml definitions.toml sp_etymology.toml etymology.toml
    let information = std::fs::read_dir("./res/sona/words/source/")
        .unwrap()
        .filter_map(|dir_entry| dir_entry.ok())
        .map(|dir_entry| {
            (
                dir_entry.file_name().to_str().unwrap().to_string(),
                dir_entry.path(),
            )
        })
        .filter_map(
            |(file_name, path)| match std::fs::read_to_string(path).ok() {
                Some(data) => Some((file_name, data)),
                None => None,
            },
        )
        .filter_map(|(file_name, data)| match data.parse::<Table>().ok() {
            Some(table) => Some((file_name, table)),
            None => None,
        })
        .collect::<HashMap<String, Table>>();

    // collect all words and compile them into a Table
    let words = std::fs::read_dir("./res/sona/words/metadata/")
        .unwrap()
        .filter_map(|dir_entry| dir_entry.ok())
        .map(|dir_entry| dir_entry.path())
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .filter_map(|data| data.parse::<Table>().ok())
        .map(|table| (table["id"].to_owned().to_string().replace("\"", ""), table))
        .map(|(word, mut table)| {
            eprintln!("{:?}", information.keys());

            let definition = information.get("definitions.toml").unwrap();
            let commentary = information.get("commentary.toml").unwrap();

            table.insert(
                "definition".into(),
                definition.get(&word).unwrap().to_owned().into(),
            );
            table.insert(
                "commentary".into(),
                commentary.get(&word).unwrap().to_owned(),
            );

            (word, table)
        })
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
