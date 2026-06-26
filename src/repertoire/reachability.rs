use std::collections::{HashSet, HashMap};
use pgn_reader::{Reader, SanPlus, Skip, Visitor};
use shakmaty::{fen::Epd, Chess, EnPassantMode, Position};
use std::ops::ControlFlow;
use super::tree::OpponentTree;

// --- Visiteur pour extraire toutes les positions du répertoire ---

struct RepertoryVisitor {
    stack: Vec<(Chess, Chess, Vec<String>)>, // pile pour gérer les variantes (position_precedente, position_courante, moves)
    positions: HashSet<String>,
    parents: HashMap<String, (String, String)>, // child_fen -> (parent_fen, san_move)
}

impl RepertoryVisitor {
    fn new() -> Self {
        RepertoryVisitor {
            stack: vec![(Chess::default(), Chess::default(), vec![])],
            positions: HashSet::new(),
            parents: HashMap::new(),
        }
    }
}

impl Visitor for RepertoryVisitor {
    type Tags = ();
    type Movetext = ();
    type Output = ();

    fn begin_tags(&mut self) -> ControlFlow<Self::Output, Self::Tags> {
        ControlFlow::Continue(())
    }

    fn begin_movetext(
        &mut self,
        _tags: Self::Tags,
    ) -> ControlFlow<Self::Output, Self::Movetext> {
        self.stack = vec![(Chess::default(), Chess::default(), vec![])];
        ControlFlow::Continue(())
    }

    fn end_game(&mut self, _movetext: Self::Movetext) -> Self::Output {}

    fn san(
        &mut self,
        _movetext: &mut Self::Movetext,
        san_plus: SanPlus,
    ) -> ControlFlow<Self::Output> {
        let (_prev_pos, current_pos, mut moves) = self.stack.last().unwrap().clone();
        
        let fen = Epd::from_position(&current_pos, EnPassantMode::Legal).to_string();
        self.positions.insert(fen.clone());

        let m = match san_plus.san.to_move(&current_pos) {
            Ok(m) => m,
            Err(_) => return ControlFlow::Break(()),
        };
        let next = {
            let mut pos = current_pos.clone();
            pos.play_unchecked(m);
            pos
        };
        
        let next_fen = Epd::from_position(&next, EnPassantMode::Legal).to_string();
        self.positions.insert(next_fen.clone());
        self.parents.insert(next_fen, (fen, san_plus.san.to_string()));

        moves.push(san_plus.san.to_string());
        *self.stack.last_mut().unwrap() = (current_pos, next, moves);
        ControlFlow::Continue(())
    }

    fn begin_variation(
        &mut self,
        _movetext: &mut Self::Movetext,
    ) -> ControlFlow<Self::Output, Skip> {
        let (prev_pos, _, moves) = self.stack.last().unwrap().clone();
        let var_moves = if moves.is_empty() {
            vec![]
        } else {
            moves[..moves.len() - 1].to_vec()
        };
        self.stack.push((prev_pos.clone(), prev_pos, var_moves));
        ControlFlow::Continue(Skip(false))
    }

    fn end_variation(
        &mut self,
        _movetext: &mut Self::Movetext,
    ) -> ControlFlow<Self::Output> {
        self.stack.pop();
        ControlFlow::Continue(())
    }
}

// --- ReachabilityEngine ---

pub struct ReachabilityResult {
    pub positions: HashSet<String>,
    pub parents: HashMap<String, (String, String)>,
}

pub struct ReachabilityEngine {
    pub max_depth: u8,
    pub _min_frequency: f64,
}

impl ReachabilityEngine {
    pub fn new(max_depth: u8, min_frequency: f64) -> Self {
        ReachabilityEngine {
            max_depth,
            _min_frequency: min_frequency,
        }
    }

    pub fn compute(
        &self,
        user_pgn: Vec<String>,
        opponent_tree: &OpponentTree,
        mask_depth: u32,
    ) -> ReachabilityResult {
        let mut reachable = HashSet::new();
        let mut parents = HashMap::new();

        // 1. Extraire toutes les positions de départ du répertoire et leurs relations
        let (_starting_positions, starting_parents) = self.extract_repertory_positions(user_pgn);
        
        // Construire la map des transitions de l'utilisateur : parent_fen -> {(child_fen, san_move)}
        let mut user_transitions: HashMap<String, HashSet<(String, String)>> = HashMap::new();
        for (child_fen, (parent_fen, san_move)) in &starting_parents {
            user_transitions
                .entry(parent_fen.clone())
                .or_default()
                .insert((child_fen.clone(), san_move.clone()));
        }

        // 2. Parcourir à partir de la position initiale
        let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -".to_string();

        self.explore_tree(
            &start_fen,
            0,
            true, // in_intersection
            mask_depth,
            &user_transitions,
            opponent_tree,
            &mut reachable,
            &mut parents,
        );

        ReachabilityResult {
            positions: reachable,
            parents,
        }
    }

    fn explore_tree(
        &self,
        fen: &str,
        depth: u32,
        in_intersection: bool,
        mask_depth: u32,
        user_transitions: &HashMap<String, HashSet<(String, String)>>,
        opponent_tree: &OpponentTree,
        reachable: &mut HashSet<String>,
        parents: &mut HashMap<String, (String, String)>,
    ) {
        if depth >= self.max_depth as u32 || reachable.contains(fen) {
            return;
        }
        reachable.insert(fen.to_string());

        let mut next_transitions = Vec::new();
        let mut next_in_intersection = in_intersection;

        if in_intersection {
            if depth >= mask_depth {
                // Seuil critique : limite du masque atteinte.
                // On passe hors de l'intersection et on explore tout l'arbre adverse.
                next_in_intersection = false;
                if let Some(opp_trans) = opponent_tree.transitions.get(fen) {
                    for t in opp_trans {
                        next_transitions.push(t.clone());
                    }
                }
            } else {
                // Dans le masque : intersection active
                let user_moves = user_transitions.get(fen);
                let opp_moves = opponent_tree.transitions.get(fen);

                if let (Some(u_moves), Some(o_moves)) = (user_moves, opp_moves) {
                    for (u_child, u_san) in u_moves {
                        if o_moves.iter().any(|(o_child, _)| o_child == u_child) {
                            next_transitions.push((u_child.clone(), u_san.clone()));
                        }
                    }
                }

                // Si l'intersection est vide, c'est une feuille d'intersection (point critique).
                // On sort de l'intersection et on suit l'arbre adverse de façon non-coupée.
                if next_transitions.is_empty() {
                    next_in_intersection = false;
                    if let Some(opp_trans) = opponent_tree.transitions.get(fen) {
                        for t in opp_trans {
                            next_transitions.push(t.clone());
                        }
                    }
                }
            }
        } else {
            // Hors intersection : suivre l'arbre adverse sans aucune coupure
            if let Some(opp_trans) = opponent_tree.transitions.get(fen) {
                for t in opp_trans {
                    next_transitions.push(t.clone());
                }
            }
        }

        // Parcourir récursivement les branches sélectionnées
        for (child_fen, san_move) in next_transitions {
            parents.entry(child_fen.clone()).or_insert_with(|| (fen.to_string(), san_move));
            self.explore_tree(
                &child_fen,
                depth + 1,
                next_in_intersection,
                mask_depth,
                user_transitions,
                opponent_tree,
                reachable,
                parents,
            );
        }
    }

    fn extract_repertory_positions(
        &self,
        user_pgn: Vec<String>,
    ) -> (HashSet<String>, HashMap<String, (String, String)>) {
        let mut visitor = RepertoryVisitor::new();
        for pgn in user_pgn {
            let mut reader = Reader::new(std::io::Cursor::new(pgn));
            let _ = reader.read_game(&mut visitor);
        }
        (visitor.positions, visitor.parents)
    }

    #[cfg(test)]
    fn apply_uci(&self, fen: &str, uci: &str) -> Option<(String, String)> {
        use shakmaty::{fen::Fen, uci::UciMove, CastlingMode, san::San};
        use std::str::FromStr;

        let fen_parsed: Fen = Fen::from_ascii(fen.as_bytes()).ok()?;
        let pos: Chess =
            fen_parsed.into_position(CastlingMode::Standard).ok()?;
        let uci_move = UciMove::from_str(uci).ok()?;
        let m = uci_move.to_move(&pos).ok()?;
        let san = San::from_move(&pos, m).to_string();
        let mut next = pos;
        next.play_unchecked(m);
        let next_fen = Epd::from_position(&next, EnPassantMode::Legal).to_string();
        Some((next_fen, san))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_uci() {
        let engine = ReachabilityEngine::new(3, 0.05);
        let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -";
        let (next, san) = engine.apply_uci(start_fen, "e2e4").unwrap();
        assert_eq!(next, "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq -");
        assert_eq!(san, "e4");
    }

    #[test]
    fn test_repertory_visitor_simple() {
        let pgn = "[Event \"Test\"]\n\n1. e4 e5 *".to_string();
        let engine = ReachabilityEngine::new(3, 0.05);
        let (positions, parents) = engine.extract_repertory_positions(vec![pgn]);
        
        let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -".to_string();
        let after_e4 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq -".to_string();
        
        assert!(positions.contains(&start_fen));
        assert!(positions.contains(&after_e4));
        
        let parent_info = parents.get(&after_e4).unwrap();
        assert_eq!(parent_info.0, start_fen);
        assert_eq!(parent_info.1, "e4");
    }

    #[test]
    fn test_repertory_visitor_variations() {
        let pgn = "[Event \"Test\"]\n\n1. e4 (1. d4 d5) e5 *".to_string();
        let engine = ReachabilityEngine::new(3, 0.05);
        let (positions, parents) = engine.extract_repertory_positions(vec![pgn]);
        
        let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -".to_string();
        let after_e4 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq -".to_string();
        let after_d4 = "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq -".to_string();
        
        assert!(positions.contains(&start_fen));
        assert!(positions.contains(&after_e4));
        assert!(positions.contains(&after_d4));

        assert_eq!(parents.get(&after_e4).unwrap().0, start_fen);
        assert_eq!(parents.get(&after_d4).unwrap().0, start_fen);
    }

    #[test]
    fn test_compute_intersection_and_opponent_tree() {
        // Répertoire utilisateur : 1. e4 e5 (s'arrête au coup 1... e5)
        let user_pgn = vec!["[Event \"User\"]\n\n1. e4 e5 *".to_string()];
        
        // Parties adverses : 
        // Partie 1 : 1. e4 e5 2. Nf3 Nc6 3. Bb5 (continue au-delà de 1... e5)
        // Partie 2 : 1. d4 d5 (ne correspond pas au début de notre répertoire)
        let opponent_pgn = vec![
            "[Event \"Opponent 1\"]\n[White \"Opp]\n[Black \"Me\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 *".to_string(),
            "[Event \"Opponent 2\"]\n[White \"Opp\"]\n[Black \"Me\"]\n\n1. d4 d5 *".to_string(),
        ];
        
        let opponent_tree = OpponentTree::build_from_pgn(opponent_pgn, "Opp".to_string());
        let engine = ReachabilityEngine::new(10, 0.0);
        
        // Calcul avec un masque de 2 (1. e4 e5)
        let result = engine.compute(user_pgn, &opponent_tree, 2);
        
        let start_fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -".to_string();
        let after_e4 = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq -".to_string();
        let after_e5 = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq -".to_string();
        let after_nf3 = "rnbqkbnr/pppp1ppp/8/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R b KQkq -".to_string();
        let after_nc6 = "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq -".to_string();
        let after_bb5 = "r1bqkbnr/pppp1ppp/2n5/1B2p3/4P3/5N2/PPPP1PPP/RNBQK2R b KQkq -".to_string();
        
        // Doit contenir les positions de l'intersection
        assert!(result.positions.contains(&start_fen));
        assert!(result.positions.contains(&after_e4));
        assert!(result.positions.contains(&after_e5));
        
        // Doit aussi contenir les positions du sous-arbre de l'adversaire (au-delà du point critique 1... e5)
        assert!(result.positions.contains(&after_nf3));
        assert!(result.positions.contains(&after_nc6));
        assert!(result.positions.contains(&after_bb5));
        
        // Ne doit pas contenir 1. d4 d5 (car hors intersection au début de la partie)
        let after_d4 = "rnbqkbnr/pppppppp/8/8/3P4/8/PPP1PPPP/RNBQKBNR b KQkq -".to_string();
        assert!(!result.positions.contains(&after_d4));
    }
}

