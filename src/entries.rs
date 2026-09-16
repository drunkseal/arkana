use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct GameEntry {
    pub id: u32,
    pub name: String,
    pub exec: String,
    pub cover: Option<String>,
}

#[derive(serde::Deserialize)]
struct TomlGame {
    name: Option<String>,
    exec: Option<String>,
    cover: Option<String>,
}

pub fn load_games(dirs: &[PathBuf]) -> Vec<GameEntry> {
    let mut games = Vec::new();
    let mut next_id: u32 = 0;

    for dir in dirs {
        let Ok(read_dir) = fs::read_dir(dir) else {
            continue;
        };

        let mut paths = Vec::new();
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("toml") {
                paths.push(path);
            }
        }
        paths.sort();

        for path in paths {
            if let Some(mut game) = parse_game(&path) {
                if !game.exec.is_empty() {
                    game.id = next_id;
                    next_id += 1;
                    games.push(game);
                }
            }
        }
    }

    if games.is_empty() {
        games.push(GameEntry {
            id: 0,
            name: "Howdy!".to_string(),
            exec: String::new(),
            cover: None,
        });
    }

    games
}

pub fn parse_game(path: &Path) -> Option<GameEntry> {
    let contents = fs::read_to_string(path).ok()?;
    parse_game_contents(&contents)
}

pub fn parse_game_contents(contents: &str) -> Option<GameEntry> {
    // Try standard TOML parsing first
    if let Ok(tg) = toml::from_str::<TomlGame>(contents) {
        if let (Some(name), Some(exec)) = (tg.name, tg.exec) {
            return Some(GameEntry {
                id: 0,
                name,
                exec,
                cover: tg.cover,
            });
        }
    }

    // Fallback to manual line-by-line parsing (matching the original C++ behavior)
    let mut name: Option<String> = None;
    let mut exec: Option<String> = None;
    let mut cover: Option<String> = None;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }

        let Some((key_part, val_part)) = line.split_once('=') else {
            continue;
        };

        let key = key_part.trim();
        let mut value = val_part.trim();
        if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
            value = &value[1..value.len() - 1];
        }

        match key {
            "name" => name = Some(value.to_string()),
            "exec" => exec = Some(value.to_string()),
            "cover" => cover = Some(value.to_string()),
            _ => {}
        }
    }

    let name = name?;
    let exec = exec?;

    Some(GameEntry {
        id: 0,
        name,
        exec,
        cover,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_standard_toml() {
        let toml_str = r#"
            name = "Sonic Adventure"
            exec = "sonic-adventure"
            cover = "/games/sonic/cover.png"
        "#;
        let game = parse_game_contents(toml_str).unwrap();
        assert_eq!(game.name, "Sonic Adventure");
        assert_eq!(game.exec, "sonic-adventure");
        assert_eq!(game.cover, Some("/games/sonic/cover.png".to_string()));
    }

    #[test]
    fn test_parse_unquoted() {
        let toml_str = "name = Sonic\nexec = sonic\n";
        let game = parse_game_contents(toml_str).unwrap();
        assert_eq!(game.name, "Sonic");
        assert_eq!(game.exec, "sonic");
        assert_eq!(game.cover, None);
    }
}
