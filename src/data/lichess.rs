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
    
    let client = reqwest::blocking::Client::builder()
        .user_agent("chess-prep/1.0 (https://github.com/DoSkill77/chess_prep)")
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let response = client
        .get(&url)
        .header("Accept", "application/x-chess-pgn")
        .send()?;

    let status = response.status();
    if status.as_u16() == 429 {
        return Err("Limite de requêtes Lichess dépassée (API Rate Limit 429). Veuillez réessayer d'ici quelques minutes.".into());
    }

    if status.as_u16() == 404 {
        return Err(format!("Joueur introuvable sur Lichess : {}", username).into());
    }

    if !status.is_success() {
        return Err(format!("Erreur retournée par Lichess (Status HTTP {})", status).into());
    }

    let bytes = response.bytes()?;
    let content = String::from_utf8_lossy(&bytes).into_owned();

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
