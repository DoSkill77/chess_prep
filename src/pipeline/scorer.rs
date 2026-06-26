use crate::config::ScoringWeights;
use crate::pipeline::analyzer::AnalyzedPosition;

pub trait ScoringCriterion {
    fn score(&self, position: &AnalyzedPosition) -> f64;
}

pub struct NoveltyScorer;

impl ScoringCriterion for NoveltyScorer {
    fn score(&self, position: &AnalyzedPosition) -> f64 {
        position.novelty_score
    }
}

pub struct ComplexityScorer;

impl ScoringCriterion for ComplexityScorer {
    fn score(&self, position: &AnalyzedPosition) -> f64 {
        if position.candidate_moves.len() < 2 {
            return 0.0;
        }

        // Gap between the best move evaluation and the second best move evaluation (in pawns)
        let e1 = position.candidate_moves[0].eval.to_f64();
        let e2 = position.candidate_moves[1].eval.to_f64();
        let gap = (e1 - e2).abs();

        // Normalized score: 10.0 if the gap is 0.0 (maximum complexity/multiple equivalent moves)
        // 0.0 if the gap is >= 2.0 pawns (simple/forced move)
        10.0 * (1.0 - (gap / 2.0).min(1.0))
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScoredLine {
    pub fen: String,
    pub best_move: String,
    pub novelty_score: f64,
    pub complexity_score: f64,
    pub final_score: f64,
    pub pv: Vec<String>,
}

pub struct Scorer {
    pub weights: ScoringWeights,
}

impl Scorer {
    pub fn new(weights: ScoringWeights) -> Self {
        Self { weights }
    }

    /// Computes novelty and complexity scores for each analyzed position,
    /// combines them with the configured weights, and returns the sorted results (best lines first).
    pub fn score(&self, analyzed_positions: &[AnalyzedPosition]) -> Vec<ScoredLine> {
        let novelty_scorer = NoveltyScorer;
        let complexity_scorer = ComplexityScorer;

        let mut scored_lines = Vec::new();

        for pos in analyzed_positions {
            let novelty = novelty_scorer.score(pos);
            let complexity = complexity_scorer.score(pos);
            let final_score = self.weights.novelty * novelty + self.weights.complexity * complexity;

            scored_lines.push(ScoredLine {
                fen: pos.fen.clone(),
                best_move: pos.best_move.clone(),
                novelty_score: novelty,
                complexity_score: complexity,
                final_score,
                pv: if pos.candidate_moves.is_empty() {
                    vec![]
                } else {
                    pos.candidate_moves[0].pv.clone()
                },
            });
        }

        // Sort by final_score descending, then FEN for determinism
        scored_lines.sort_by(|a, b| {
            b.final_score
                .total_cmp(&a.final_score)
                .then_with(|| a.fen.cmp(&b.fen))
        });

        scored_lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::stockfish::{Evaluation, ScoredMove};

    #[test]
    fn test_novelty_scorer() {
        let pos = AnalyzedPosition {
            fen: "start".to_string(),
            opponent_count: 0,
            novelty_score: 9.5,
            eval: Evaluation::Cp(0),
            best_move: "e2e4".to_string(),
            candidate_moves: vec![],
        };
        let scorer = NoveltyScorer;
        assert_eq!(scorer.score(&pos), 9.5);
    }

    #[test]
    fn test_complexity_scorer() {
        let scorer = ComplexityScorer;

        // Forced position (only 1 candidate move)
        let forced = AnalyzedPosition {
            fen: "pos1".to_string(),
            opponent_count: 0,
            novelty_score: 5.0,
            eval: Evaluation::Cp(10),
            best_move: "e2e4".to_string(),
            candidate_moves: vec![
                ScoredMove { uci: "e2e4".to_string(), eval: Evaluation::Cp(10), pv: vec!["e2e4".to_string()] }
            ],
        };
        assert_eq!(scorer.score(&forced), 0.0);

        // Gap of 0.0 (maximum complexity)
        let complex = AnalyzedPosition {
            fen: "pos2".to_string(),
            opponent_count: 0,
            novelty_score: 5.0,
            eval: Evaluation::Cp(10),
            best_move: "e2e4".to_string(),
            candidate_moves: vec![
                ScoredMove { uci: "e2e4".to_string(), eval: Evaluation::Cp(10), pv: vec!["e2e4".to_string()] },
                ScoredMove { uci: "d2d4".to_string(), eval: Evaluation::Cp(10), pv: vec!["d2d4".to_string()] }
            ],
        };
        assert_eq!(scorer.score(&complex), 10.0);

        // Gap of 1.0 pawn (complexity 5.0)
        let medium = AnalyzedPosition {
            fen: "pos3".to_string(),
            opponent_count: 0,
            novelty_score: 5.0,
            eval: Evaluation::Cp(110),
            best_move: "e2e4".to_string(),
            candidate_moves: vec![
                ScoredMove { uci: "e2e4".to_string(), eval: Evaluation::Cp(110), pv: vec!["e2e4".to_string()] },
                ScoredMove { uci: "d2d4".to_string(), eval: Evaluation::Cp(10), pv: vec!["d2d4".to_string()] }
            ],
        };
        assert_eq!(scorer.score(&medium), 5.0);
    }

    #[test]
    fn test_scorer_integration() {
        let weights = ScoringWeights {
            novelty: 0.60,
            complexity: 0.40,
        };
        let scorer = Scorer::new(weights);

        let positions = vec![
            AnalyzedPosition {
                fen: "pos1".to_string(),
                opponent_count: 0,
                novelty_score: 10.0,
                eval: Evaluation::Cp(10),
                best_move: "e2e4".to_string(),
                candidate_moves: vec![
                    ScoredMove { uci: "e2e4".to_string(), eval: Evaluation::Cp(10), pv: vec!["e2e4".to_string()] },
                    ScoredMove { uci: "d2d4".to_string(), eval: Evaluation::Cp(10), pv: vec!["d2d4".to_string()] }
                ], // Complexity: 10.0
            }, // final score: 0.6 * 10 + 0.4 * 10 = 10.0
            AnalyzedPosition {
                fen: "pos2".to_string(),
                opponent_count: 0,
                novelty_score: 5.0,
                eval: Evaluation::Cp(10),
                best_move: "e2e4".to_string(),
                candidate_moves: vec![
                    ScoredMove { uci: "e2e4".to_string(), eval: Evaluation::Cp(10), pv: vec!["e2e4".to_string()] }
                ], // Complexity: 0.0
            } // final score: 0.6 * 5 + 0.4 * 0 = 3.0
        ];

        let results = scorer.score(&positions);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].fen, "pos1");
        assert_eq!(results[0].final_score, 10.0);
        assert_eq!(results[1].fen, "pos2");
        assert_eq!(results[1].final_score, 3.0);
    }
}
