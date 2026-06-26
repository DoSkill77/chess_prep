use std::collections::{HashSet, HashMap};
use crate::repertoire::tree::OpponentTree;

#[derive(Debug, Clone, PartialEq)]
pub struct ProfiledPosition {
    pub fen: String,
    pub opponent_count: u32,
    pub novelty_score: f64,
}

pub struct Profiler {
    pub novelty_threshold: u32,
    pub min_novelty_ply: u32,
    pub min_parent_count: u32,
}

impl Profiler {
    pub fn new(novelty_threshold: u32, min_novelty_ply: u32, min_parent_count: u32) -> Self {
        Self {
            novelty_threshold,
            min_novelty_ply,
            min_parent_count,
        }
    }

    /// Crosses reachable positions with the OpponentTree.
    /// Filters out early game moves (based on min_novelty_ply).
    /// Keeps only positions where the opponent's count is strictly below the novelty_threshold,
    /// AND where the parent position's count is >= novelty_threshold AND >= min_parent_count.
    pub fn profile(
        &self,
        reachable_positions: &HashSet<String>,
        parents: &HashMap<String, (String, String)>,
        opponent_tree: &OpponentTree,
    ) -> Vec<ProfiledPosition> {
        let mut profiles = Vec::new();

        for fen in reachable_positions {
            // Filter 1: Check depth (ply count)
            let ply = get_ply_count(fen, parents);
            if ply < self.min_novelty_ply {
                continue;
            }

            let opponent_count = opponent_tree.get_count(fen);
            
            // Only keep positions under the novelty threshold
            if opponent_count < self.novelty_threshold {
                // Filter 2: Check if it's a deviation point (parent count >= threshold and >= min_parent_count)
                if let Some((parent_fen, _)) = parents.get(fen) {
                    let parent_count = opponent_tree.get_count(parent_fen);
                    
                    let min_p_required = self.min_parent_count.max(self.novelty_threshold);
                    if parent_count >= min_p_required {
                        let novelty_score = if self.novelty_threshold == 0 {
                            0.0
                        } else {
                            10.0 * (1.0 - (opponent_count as f64) / (self.novelty_threshold as f64 * 3.0))
                        };

                        profiles.push(ProfiledPosition {
                            fen: fen.clone(),
                            opponent_count,
                            novelty_score,
                        });
                    }
                }
            }
        }

        // Sort by novelty_score descending (most novel first), then opponent_count ascending, then FEN
        profiles.sort_by(|a, b| {
            b.novelty_score
                .total_cmp(&a.novelty_score)
                .then_with(|| a.opponent_count.cmp(&b.opponent_count))
                .then_with(|| a.fen.cmp(&b.fen))
        });

        profiles
    }
}

fn get_ply_count(
    fen: &str,
    parents: &HashMap<String, (String, String)>,
) -> u32 {
    let mut count = 0;
    let mut current = fen.to_string();
    while let Some((parent, _)) = parents.get(&current) {
        count += 1;
        current = parent.clone();
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiler_novelty_score() {
        let pgn = vec![
            "[White \"Magnus\"]\n[Black \"Doria\"]\n\n1. e4 *".to_string(),
            "[White \"Magnus\"]\n[Black \"Doria\"]\n\n1. e4 *".to_string(),
        ];
        let opponent_tree = OpponentTree::build_from_pgn(pgn, "Magnus".to_string());

        let fen_start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -".to_string();
        let fen_novelty = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq -".to_string(); // Position after 1. e4

        let mut reachable = HashSet::new();
        reachable.insert(fen_start.clone());
        reachable.insert(fen_novelty.clone());

        let mut parents = HashMap::new();
        parents.insert(fen_novelty.clone(), (fen_start.clone(), "e4".to_string()));

        // Magnus played e4 twice from starting position, so fen_start should have count 2.
        // Magnus has never faced fen_novelty as the player to move (it is Black's turn).

        let profiler = Profiler::new(2, 0, 1);
        let profiles = profiler.profile(&reachable, &parents, &opponent_tree);

        // Parent (start FEN) has count 2 >= min_p_required (2).
        // fen_novelty has count 0 < novelty_threshold (2).
        // So fen_novelty is a deviation point.
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].fen, fen_novelty);
    }

    #[test]
    fn test_profiler_deviation_filtering() {
        let pgn = vec![
            "[White \"Magnus\"]\n[Black \"Doria\"]\n\n1. e4 e5 *".to_string(),
            "[White \"Magnus\"]\n[Black \"Doria\"]\n\n1. e4 e5 *".to_string(),
            "[White \"Magnus\"]\n[Black \"Doria\"]\n\n1. e4 e5 *".to_string(),
            "[White \"Magnus\"]\n[Black \"Doria\"]\n\n1. e4 e5 *".to_string(),
            "[White \"Magnus\"]\n[Black \"Doria\"]\n\n1. e4 e5 *".to_string(),
        ];
        let opponent_tree = OpponentTree::build_from_pgn(pgn, "Magnus".to_string());
        
        let fen_start = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -".to_string();
        let fen_after_e4 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq -".to_string();
        let fen_after_e5 = "rnbqkbnr/ppppqbpp/8/8/4P3/8/PPPP1PPP/RNBQKBNR w KQkq -".to_string(); // fictitious novelty

        let mut reachable = HashSet::new();
        reachable.insert(fen_start.clone());
        reachable.insert(fen_after_e4.clone());
        reachable.insert(fen_after_e5.clone());

        let mut parents = HashMap::new();
        parents.insert(fen_after_e4.clone(), (fen_start.clone(), "e4".to_string()));
        parents.insert(fen_after_e5.clone(), (fen_after_e4.clone(), "Qe7".to_string()));

        // threshold = 5
        // start: count 5 >= 5 (not novelty)
        // after_e4: count 5 >= 5 (not novelty)
        // after_e5: count 0 < 5, parent after_e4 has count 5 >= 5 -> is_deviation = true!
        let profiler = Profiler::new(5, 0, 0);
        let profiles = profiler.profile(&reachable, &parents, &opponent_tree);

        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].fen, fen_after_e5);
    }

    #[test]
    fn test_profiler_strict_filters() {
        let pgn = vec![
            "[White \"Magnus\"]\n[Black \"Doria\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 *".to_string(),
        ];
        let opponent_tree = OpponentTree::build_from_pgn(pgn, "Magnus".to_string());
        
        let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -".to_string();
        let fen_after_e4 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq -".to_string(); // ply 1
        let fen_after_e5 = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq -".to_string(); // ply 2
        let fen_after_nf3 = "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq -".to_string(); // ply 3
        let fen_after_nc6 = "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq -".to_string(); // ply 4
        let fen_after_bb5 = "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R b KQkq -".to_string(); // ply 5
        let fen_novelty = "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R b KQkq - a6".to_string(); // fictitious novelty, ply 6

        let mut reachable = HashSet::new();
        reachable.insert(fen_after_e4.clone());
        reachable.insert(fen_novelty.clone());

        let mut parents = HashMap::new();
        parents.insert(fen_after_e4.clone(), (start_fen.clone(), "e4".to_string()));
        parents.insert(fen_novelty.clone(), (fen_after_bb5.clone(), "a6".to_string()));
        parents.insert(fen_after_bb5.clone(), (fen_after_nc6.clone(), "Bb5".to_string()));
        parents.insert(fen_after_nc6.clone(), (fen_after_nf3.clone(), "Nc6".to_string()));
        parents.insert(fen_after_nf3.clone(), (fen_after_e5.clone(), "Nf3".to_string()));
        parents.insert(fen_after_e5.clone(), (fen_after_e4.clone(), "e5".to_string()));

        // 1. Test ply count restriction: min_novelty_ply = 6
        // start_fen is parent of e4 (ply 0), e4 is ply 1.
        // e4 should be skipped because ply = 1 < 6.
        // novelty should be evaluated because ply = 6 >= 6.
        let profiler = Profiler::new(1, 6, 1);
        let profiles = profiler.profile(&reachable, &parents, &opponent_tree);
        // Only fen_novelty should remain
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].fen, fen_novelty);

        // 2. Test min_parent_count restriction
        // Let's set min_parent_count = 5.
        // Magnus only played 1 game, so parent count is 1.
        // Since 1 < 5, fen_novelty should be skipped!
        let profiler_strict = Profiler::new(1, 6, 5);
        let profiles_strict = profiler_strict.profile(&reachable, &parents, &opponent_tree);
        assert_eq!(profiles_strict.len(), 0);
    }
}
