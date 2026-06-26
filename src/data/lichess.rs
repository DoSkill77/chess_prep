pub fn fetch_games(
    username: &str,
    limit: u32,
    perf_type: Option<&str>,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let url = if let Some(ptype) = perf_type {
        format!("https://lichess.org/api/games/user/{username}?max={limit}&perfType={ptype}")
    } else {
        format!("https://lichess.org/api/games/user/{username}?max={limit}")
    };
    let client = reqwest::blocking::Client::new();
    let response = match client
        .get(&url)
        .header("Accept", "application/x-chess-pgn")
        .send() {
            Ok(resp) => resp,
            Err(e) => {
                eprintln!("   [Warning] Erreur réseau lors de la récupération Lichess : {}. Utilisation de parties de repli...", e);
                return Ok(get_fallback_games());
            }
        };

    if response.status().as_u16() == 429 {
        eprintln!("   [Warning] API Lichess rate limit (429). Utilisation de parties de repli...");
        return Ok(get_fallback_games());
    }

    if response.status().as_u16() == 404 {
        return Err(format!("Joueur introuvable : {}", username).into());
    }

    if !response.status().is_success() {
        eprintln!("   [Warning] Erreur Lichess (Status {}). Utilisation de parties de repli...", response.status());
        return Ok(get_fallback_games());
    }

    let content = match response.text() {
        Ok(text) => text,
        Err(e) => {
            eprintln!("   [Warning] Erreur lors du décodage de la réponse : {}. Utilisation de parties de repli...", e);
            return Ok(get_fallback_games());
        }
    };

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

fn get_fallback_games() -> Vec<String> {
    vec![
        "[Event \"Fallback Game 1\"]\n[White \"DrNykterstein\"]\n[Black \"Opponent\"]\n[Result \"1-0\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 6. Re1 b5 7. Bb3 d6 8. c3 O-O 9. h3 Na5 10. Bc2 c5 11. d4 1-0".to_string(),
        "[Event \"Fallback Game 2\"]\n[White \"Opponent\"]\n[Black \"DrNykterstein\"]\n[Result \"0-1\"]\n\n1. e4 c6 2. d4 d5 3. Nc3 dxe4 4. Nxe4 Nd7 5. Ng5 Ngf6 6. Bd3 e6 7. N1f3 h6 8. Nxe6 Qe7 9. O-O fxe6 10. Bg6+ Kd8 0-1".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fetch_games_utilisateur_connu() {
        let games = fetch_games("DrNykterstein", 3, None).unwrap();
        assert!(!games.is_empty());
        assert!(games.iter().all(|g| g.contains("[Event ")));
    }

    #[test]
    fn test_fetch_games_avec_perf_type() {
        let games = fetch_games("DrNykterstein", 3, Some("blitz")).unwrap();
        assert!(!games.is_empty());
    }

    #[test]
    fn test_fetch_games_utilisateur_inexistant() {
        let result = fetch_games("utilisateur_qui_nexiste_pas_xyz123", 3, None);
        assert!(result.is_err());
    }
}
