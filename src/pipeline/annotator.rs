use crate::pipeline::scorer::ScoredLine;
use std::collections::HashMap;

pub trait Annotator {
    fn annotate(&self, user_pgn: &str, scored_lines: &[ScoredLine]) -> Result<String, Box<dyn std::error::Error>>;
}

pub struct BasicAnnotator {
    pub opponent: String,
    pub parents: HashMap<String, (String, String)>,
}

impl Annotator for BasicAnnotator {
    fn annotate(&self, _user_pgn: &str, scored_lines: &[ScoredLine]) -> Result<String, Box<dyn std::error::Error>> {
        let mut pgn_output = String::new();

        for (i, line) in scored_lines.iter().enumerate() {
            // 1. Reconstruct path of SAN moves leading to line.fen
            let mut path_moves = Vec::new();
            let mut current_fen = line.fen.clone();
            while let Some((parent_fen, san_move)) = self.parents.get(&current_fen) {
                path_moves.push(san_move.clone());
                current_fen = parent_fen.clone();
            }
            path_moves.reverse();

            // 2. Convert Stockfish PV continuation to SAN moves
            let pv_san = uci_pv_to_san(&line.fen, &line.pv);

            // 3. Format moves into a single sequence
            let mut formatted_pgn = String::new();
            let mut all_moves = path_moves.clone();
            
            let novelty_idx = all_moves.len();
            if !pv_san.is_empty() {
                all_moves.extend(pv_san.clone());
            } else {
                all_moves.push(line.best_move.clone());
            }

            for (idx, mv) in all_moves.iter().enumerate() {
                let move_num = idx / 2 + 1;
                let is_white = idx % 2 == 0;
                
                if is_white {
                    formatted_pgn.push_str(&format!("{}. {} ", move_num, mv));
                } else {
                    formatted_pgn.push_str(&format!("{} ", mv));
                }

                if idx == novelty_idx {
                    formatted_pgn.push_str(&format!("{{CP: {:.2} (N: {:.2}, C: {:.2})}} ", 
                        line.final_score, line.novelty_score, line.complexity_score));
                }
            }
            formatted_pgn.push_str("*");

            // 4. Determine White and Black players
            let opponent_is_white = crate::pipeline::analyzer::is_opponent_white(&line.fen);
            let white_player = if opponent_is_white { &self.opponent } else { "User" };
            let black_player = if opponent_is_white { "User" } else { &self.opponent };

            // 5. Append headers and moves to pgn_output
            pgn_output.push_str(&format!("[Event \"Prep vs {} - Candidate #{}\"]\n", self.opponent, i + 1));
            pgn_output.push_str("[Site \"chess-prep\"]\n");
            pgn_output.push_str("[Date \"2026.06.26\"]\n");
            pgn_output.push_str(&format!("[White \"{}\"]\n", white_player));
            pgn_output.push_str(&format!("[Black \"{}\"]\n", black_player));
            pgn_output.push_str("[Result \"*\"]\n");
            pgn_output.push_str(&format!("[Score \"{:.2}\"]\n", line.final_score));
            pgn_output.push_str(&format!("[Novelty \"{:.2}\"]\n", line.novelty_score));
            pgn_output.push_str(&format!("[Complexity \"{:.2}\"]\n", line.complexity_score));
            pgn_output.push_str("\n");
            pgn_output.push_str(&formatted_pgn);
            pgn_output.push_str("\n\n");
        }

        Ok(pgn_output)
    }
}

pub fn uci_pv_to_san(fen: &str, pv: &[String]) -> Vec<String> {
    use shakmaty::{fen::Fen, uci::UciMove, CastlingMode, san::San, Chess, Position};
    use std::str::FromStr;

    let mut san_moves = Vec::new();
    let mut pos = match Fen::from_ascii(fen.as_bytes()) {
        Ok(f) => match f.into_position::<Chess>(CastlingMode::Standard) {
            Ok(p) => p,
            Err(_) => return pv.to_vec(),
        },
        Err(_) => return pv.to_vec(),
    };

    for uci in pv {
        if let Ok(uci_move) = UciMove::from_str(uci) {
            if let Ok(m) = uci_move.to_move(&pos) {
                let san = San::from_move(&pos, m).to_string();
                san_moves.push(san);
                pos.play_unchecked(m);
            } else {
                break;
            }
        } else {
            break;
        }
    }
    san_moves
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_annotator() {
        let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -".to_string();
        let scored_lines = vec![
            ScoredLine {
                fen: start_fen,
                best_move: "e2e4".to_string(),
                novelty_score: 10.0,
                complexity_score: 5.0,
                final_score: 8.0,
                pv: vec!["e2e4".to_string(), "e7e5".to_string()],
            }
        ];

        let parents = HashMap::new();
        let annotator = BasicAnnotator {
            opponent: "Deep Blue".to_string(),
            parents,
        };
        let annotated_pgn = annotator.annotate("", &scored_lines).unwrap();

        assert!(annotated_pgn.contains("[Event \"Prep vs Deep Blue - Candidate #1\"]"));
        assert!(annotated_pgn.contains("1. e4 {CP: 8.00 (N: 10.00, C: 5.00)} e5 *"));
    }

    #[test]
    fn test_basic_annotator_with_parents() {
        let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -".to_string();
        let after_e4 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq -".to_string();
        
        let scored_lines = vec![
            ScoredLine {
                fen: after_e4.clone(),
                best_move: "e7e5".to_string(),
                novelty_score: 10.0,
                complexity_score: 5.0,
                final_score: 8.0,
                pv: vec!["e7e5".to_string(), "g1f3".to_string()],
            }
        ];

        let mut parents = HashMap::new();
        parents.insert(after_e4, (start_fen, "e4".to_string()));

        let annotator = BasicAnnotator {
            opponent: "Deep Blue".to_string(),
            parents,
        };
        let annotated_pgn = annotator.annotate("", &scored_lines).unwrap();

        // Path is e4. Novelty is e5. PV has e5, Nf3.
        // Full moves: e4 (idx 0), e5 (idx 1, novelty), Nf3 (idx 2).
        // Formatted moves: 1. e4 e5 {CP: 8.00 ...} 2. Nf3 *
        assert!(annotated_pgn.contains("1. e4 e5 {CP: 8.00 (N: 10.00, C: 5.00)} 2. Nf3 *"));
    }
}
