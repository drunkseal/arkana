use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct GameEntry {
    pub id: u32,
    pub name: String,
    pub exec: String,
    pub cover: Option<PathBuf>,
}

pub fn load_games(dirs: Vec<PathBuf>) -> Vec<GameEntry> {
    let mut games = Vec::new();

    for dir in dirs {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();

            // Only process .toml files
            if path.extension().and_then(|s| s.to_str()) == Some("toml")
                && let Some(game) = parse_game(&path)
                && !game.exec.is_empty()
            {
                games.push(GameEntry {
                    id: games.len() as u32,
                    name: game.name,
                    exec: game.exec,
                    cover: game.cover.map(PathBuf::from),
                });
            }
        }
    }
    if games.is_empty() {
        vec![GameEntry {
            id: 0,
            name: String::from("Howdy!"),
            exec: String::from(""),
            cover: None,
        }]
    } else {
        games
    }
}

struct RawGame {
    name: String,
    exec: String,
    cover: Option<String>,
}

fn parse_game(path: &Path) -> Option<RawGame> {
    let contents = std::fs::read_to_string(path).ok()?;

    let mut name = None;
    let mut exec = None;
    let mut cover = None;

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim().trim_start_matches('"').trim_end_matches('"');

        match key {
            "name" => name = Some(value.to_string()),
            "exec" => exec = Some(value.to_string()),
            "cover" => cover = Some(value.to_string()),
            _ => {}
        }
    }

    Some(RawGame {
        name: name?,
        exec: exec?,
        cover,
    })
}