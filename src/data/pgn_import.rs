use std::path::Path;
pub fn load_pgn_file(
    path: &Path,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;

    let games: Vec<String> = content
        .split("\n[Event ")
        .enumerate()
        .map(|(i, s)| {
            if i == 0 {
                s.to_string()
            } else {
                format!("[Event {}", s)
            }
        })
        .filter(|s| s.contains("[Event "))
        .collect();
    Ok(games)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_pgn_une_partie() {
        let path = std::path::Path::new("/tmp/test_une_partie.pgn");
        std::fs::write(path, "[Event \"Test\"]\n[Site \"?\"]\n\n1. e4 e5 *\n")
            .unwrap();
        let games = load_pgn_file(path).unwrap();
        assert_eq!(games.len(), 1);
    }

    #[test]
    fn test_load_pgn_plusieurs_parties() {
        let path = std::path::Path::new("/tmp/test_plusieurs_parties.pgn");
        std::fs::write(path, "[Event \"Test1\"]\n[Site \"?\"]\n\n1. e4 e5 *\n\n[Event \"Test2\"]\n[Site \"?\"]\n\n1. d4 d5 *\n").unwrap();
        let games = load_pgn_file(path).unwrap();
        assert_eq!(games.len(), 2);
    }

    #[test]
    fn test_load_pgn_fichier_inexistant() {
        let path = std::path::Path::new("/tmp/fichier_qui_nexiste_pas.pgn");
        let result = load_pgn_file(path);
        assert!(result.is_err());
    }
}
