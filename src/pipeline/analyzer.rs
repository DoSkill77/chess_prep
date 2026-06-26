use crate::config::Config;
use crate::engine::stockfish::{find_stockfish_binary, Evaluation, ScoredMove, StockfishClient};
use crate::pipeline::profiler::ProfiledPosition;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AnalyzedPosition {
    pub fen: String,
    pub opponent_count: u32,
    pub novelty_score: f64,
    pub eval: Evaluation,
    pub best_move: String,
    pub candidate_moves: Vec<ScoredMove>,
}

pub struct Analyzer {
    pub config: Config,
    stockfish_path: String,
}

impl Analyzer {
    pub fn new(config: Config) -> Self {
        let stockfish_path = find_stockfish_binary();
        Self {
            config,
            stockfish_path,
        }
    }

    #[cfg(test)]
    pub fn with_stockfish_path(config: Config, path: String) -> Self {
        Self {
            config,
            stockfish_path: path,
        }
    }

    /// Analyzes profiled positions.
    /// 1. Runs Stockfish at filter depth.
    /// 2. Checks if the position evaluation from the user's perspective is acceptable (satisfies eval_bounds).
    /// 3. If acceptable, runs Stockfish at validate depth and records candidate moves.
    /// 4. Discards unacceptable positions.
    pub fn analyze(&self, profiled_positions: &[ProfiledPosition]) -> Result<Vec<AnalyzedPosition>, std::io::Error> {
        let mut client = StockfishClient::new(&self.stockfish_path)?;
        let mut analyzed = Vec::new();

        for pos in profiled_positions {
            // 1. Filter phase (fast evaluation)
            let filter_moves = client.analyze_position(&pos.fen, self.config.stockfish_depth_filter)?;
            if filter_moves.is_empty() {
                continue;
            }

            let best_eval = &filter_moves[0].eval;
            let opponent_is_white = is_opponent_white(&pos.fen);

            if is_acceptable_eval(best_eval, opponent_is_white, &self.config.eval_bounds) {
                // 2. Validation phase (deep evaluation)
                let validate_moves = client.analyze_position(&pos.fen, self.config.stockfish_depth_validate)?;
                if validate_moves.is_empty() {
                    continue;
                }

                let final_eval = validate_moves[0].eval.clone();
                let best_move = validate_moves[0].uci.clone();

                analyzed.push(AnalyzedPosition {
                    fen: pos.fen.clone(),
                    opponent_count: pos.opponent_count,
                    novelty_score: pos.novelty_score,
                    eval: final_eval,
                    best_move,
                    candidate_moves: validate_moves,
                });
            }
        }

        Ok(analyzed)
    }
}

pub fn is_opponent_white(fen: &str) -> bool {
    if let Some(turn) = fen.split_whitespace().nth(1) {
        turn == "w"
    } else {
        false
    }
}

pub fn is_acceptable_eval(eval: &Evaluation, opponent_is_white: bool, bounds: &crate::config::EvalBounds) -> bool {
    let score_side_to_move = eval.to_f64();
    let user_score = -score_side_to_move;
    if opponent_is_white {
        // Opponent is White, meaning user is Black. Check against Black bound.
        user_score >= bounds.black_min
    } else {
        // Opponent is Black, meaning user is White. Check against White bound.
        user_score >= bounds.white_min
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EvalBounds;

    #[test]
    fn test_is_opponent_white() {
        assert!(is_opponent_white("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -"));
        assert!(!is_opponent_white("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq -"));
    }

    #[test]
    fn test_is_acceptable_eval() {
        let bounds = EvalBounds {
            white_min: 0.0,
            black_min: -0.5,
        };

        // Case 1: Opponent is Black (user is White)
        // Opponent's score is +0.3 (Black is better by +0.3).
        // User's score (White) is -0.3. Since -0.3 < 0.0, it is unacceptable.
        assert!(!is_acceptable_eval(&Evaluation::Cp(30), false, &bounds));

        // Opponent's score is -0.1 (Black is worse by -0.1, meaning White is +0.1).
        // User's score (White) is +0.1 >= 0.0, acceptable.
        assert!(is_acceptable_eval(&Evaluation::Cp(-10), false, &bounds));

        // Case 2: Opponent is White (user is Black)
        // Opponent's score is +0.3 (White is better by +0.3).
        // User's score (Black) is -0.3. Since -0.3 >= -0.5, acceptable.
        assert!(is_acceptable_eval(&Evaluation::Cp(30), true, &bounds));

        // Opponent's score is +0.8 (White is better by +0.8).
        // User's score (Black) is -0.8. Since -0.8 < -0.5, unacceptable.
        assert!(!is_acceptable_eval(&Evaluation::Cp(80), true, &bounds));
    }

    #[test]
    fn test_analyzer_integration() {
        let path = find_stockfish_binary();
        if path == "stockfish" && !std::process::Command::new("stockfish").arg("--version").status().is_ok() {
            return;
        }

        let mut config = Config::default();
        config.stockfish_depth_filter = 6;
        config.stockfish_depth_validate = 8;
        config.eval_bounds = EvalBounds {
            white_min: -2.0,
            black_min: -2.0,
        };

        let analyzer = Analyzer::with_stockfish_path(config, path);

        let profiled = vec![
            // Starting position (opponent is White, user is Black)
            // Stockfish should evaluate it around +0.1 to +0.4 for White, meaning -0.4 to -0.1 for Black, which is >= -0.5 (acceptable)
            ProfiledPosition {
                fen: "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -".to_string(),
                opponent_count: 2,
                novelty_score: 8.0,
            }
        ];

        let results = analyzer.analyze(&profiled).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].fen, profiled[0].fen);
        assert_eq!(results[0].best_move.len(), 4);
    }
}
