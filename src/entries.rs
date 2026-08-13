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
    parse_game_contents(&contents)
}

/// Parse a `.toml`-ish `key = "value"` document.
///
/// Only `name`, `exec` and `cover` are read; the `[game]` section header and
/// any other keys are ignored. Values are split at the *first* `=` so that
/// `=` characters inside a value survive.
fn parse_game_contents(contents: &str) -> Option<RawGame> {
    let mut name = None;
    let mut exec = None;
    let mut cover = None;

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some(eq) = line.find('=') else {
            continue;
        };
        let key = line[..eq].trim();
        let value = line[eq + 1..]
            .trim()
            .trim_start_matches('"')
            .trim_end_matches('"');

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_file() {
        let game =
            parse_game_contents("name = \"Crash\"\nexec = \"crash\"\ncover = \"cover.png\"\n")
                .expect("game");
        assert_eq!(game.name, "Crash");
        assert_eq!(game.exec, "crash");
        assert_eq!(game.cover.as_deref(), Some("cover.png"));
    }

    #[test]
    fn keeps_equals_inside_values() {
        let game = parse_game_contents(
            "name = \"C=0\"\nexec = \"app --opt=x=y\"\ncover = \"/g/a=b/c.png\"\n",
        )
        .expect("game");
        assert_eq!(game.name, "C=0");
        assert_eq!(game.exec, "app --opt=x=y");
        assert_eq!(game.cover.as_deref(), Some("/g/a=b/c.png"));
    }

    #[test]
    fn ignores_comments_section_headers_and_unknown_keys() {
        let game = parse_game_contents(
            "# comment\n[game]\nother = \"ignored\"\nname = \"Ok\"\nexec = \"ok\"\n",
        )
        .expect("game");
        assert_eq!(game.name, "Ok");
        assert_eq!(game.exec, "ok");
        assert_eq!(game.cover, None);
    }

    #[test]
    fn missing_required_fields_rejected() {
        assert!(parse_game_contents("name = \"OnlyName\"\n").is_none());
        assert!(parse_game_contents("exec = \"onlyExec\"\n").is_none());
        assert!(parse_game_contents("").is_none());
    }

    #[test]
    fn identical_parts_when_equal_in_value() {
        let game = parse_game_contents("name = \"a=b\"\nexec = \"a=b\"\n").expect("game");
        assert_eq!(game.name, "a=b");
        assert_eq!(game.exec, "a=b");
    }
}