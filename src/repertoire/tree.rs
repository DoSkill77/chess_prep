use pgn_reader::{Reader, SanPlus, Visitor};
use std::{collections::{HashMap, HashSet}, io, ops::ControlFlow};

use shakmaty::{
    fen::Epd,
    Chess,
    Color::{self, Black, White},
    EnPassantMode, Position,
};

pub struct OpponentTree {
    pub positions: HashMap<String, u32>,
    pub transitions: HashMap<String, HashSet<(String, String)>>, // parent_fen -> {(child_fen, san_move)}
}

pub struct PgnVisitor {
    position: Chess,
    map: HashMap<String, u32>,
    transitions: HashMap<String, HashSet<(String, String)>>,
    opponent_name: String,
    opponent_color: Option<Color>,
}

impl Visitor for PgnVisitor {
    type Tags = ();
    type Movetext = ();
    type Output = ();
    fn begin_tags(
        &mut self,
    ) -> std::ops::ControlFlow<Self::Output, Self::Tags> {
        ControlFlow::Continue(())
    }
    fn begin_movetext(
        &mut self,
        _tags: Self::Tags,
    ) -> ControlFlow<Self::Output, Self::Movetext> {
        ControlFlow::Continue(())
    }
    fn end_game(&mut self, _movetext: Self::Movetext) -> Self::Output {
        self.position = Chess::default();
    }

    fn tag(
        &mut self,
        _tags: &mut Self::Tags,
        name: &[u8],
        value: pgn_reader::RawTag<'_>,
    ) -> ControlFlow<Self::Output> {
        if name == b"White" {
            if value.decode().as_ref() == self.opponent_name.as_bytes() {
                self.opponent_color = Some(White);
            } else {
                self.opponent_color = Some(Black);
            }
        }
        ControlFlow::Continue(())
    }

    fn san(
        &mut self,
        _movetext: &mut Self::Movetext,
        san_plus: SanPlus,
    ) -> ControlFlow<Self::Output> {
        let m = match san_plus.san.to_move(&self.position) {
            Ok(m) => m,
            Err(_) => return ControlFlow::Break(()),
        };

        let parent_fen = Epd::from_position(&self.position, EnPassantMode::Legal)
            .to_string();
        *self.map.entry(parent_fen.clone()).or_insert(0) += 1;

        self.position.play_unchecked(m);

        let child_fen = Epd::from_position(&self.position, EnPassantMode::Legal)
            .to_string();
        let san_move = san_plus.san.to_string();

        self.transitions
            .entry(parent_fen)
            .or_default()
            .insert((child_fen, san_move));

        ControlFlow::Continue(())
    }
}

impl OpponentTree {
    pub fn build_from_pgn(
        games: Vec<String>,
        opponent_name: String,
    ) -> OpponentTree {
        let mut visitor = PgnVisitor {
            position: Chess::new(),
            map: HashMap::new(),
            transitions: HashMap::new(),
            opponent_name,
            opponent_color: None,
        };
        games.iter().for_each(|game| {
            let mut reader = Reader::new(io::Cursor::new(&game));
            reader.read_game(&mut visitor).unwrap();
        });
        OpponentTree {
            positions: visitor.map,
            transitions: visitor.transitions,
        }
    }

    pub fn get_count(&self, fen: &str) -> u32 {
        *self.positions.get(fen).unwrap_or(&0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_apres_e4() {
        let pgn = vec![
            "[White \"Magnus\"]\n[Black \"Doria\"]\n\n1. e4 *".to_string()
        ];
        let tree = OpponentTree::build_from_pgn(pgn, "Magnus".to_string());
        // Magnus joue e4 depuis la position de départ — c'est cette position qui est enregistrée
        let fen =
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -".to_string();
        assert_eq!(tree.positions.get(&fen), Some(&1));
    }

    #[test]
    fn test_deux_parties_meme_position() {
        let pgn = vec![
            "[White \"Magnus\"]\n[Black \"Doria\"]\n\n1. e4 *".to_string(),
            "[White \"Magnus\"]\n[Black \"Doria\"]\n\n1. e4 *".to_string(),
        ];
        let tree = OpponentTree::build_from_pgn(pgn, "Magnus".to_string());
        let fen =
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -".to_string();
        assert_eq!(tree.positions.get(&fen), Some(&2));
    }

    #[test]
    fn test_positions_partagees_deux_parties() {
        let pgn = vec![
            "[White \"Magnus\"]\n[Black \"Doria\"]\n\n1. e4 e5 2. Nf3 *"
                .to_string(),
            "[White \"Magnus\"]\n[Black \"Doria\"]\n\n1. e4 e6 2. Nf3 *"
                .to_string(),
        ];
        let tree = OpponentTree::build_from_pgn(pgn, "Magnus".to_string());
        // Position de départ — dans les deux parties
        let fen_start =
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -".to_string();
        assert_eq!(tree.positions.get(&fen_start), Some(&2));
        // Après 1.e4 e5 — Magnus joue Nf3 depuis cette position, une seule partie
        let fen_e5 = "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq -"
            .to_string();
        assert_eq!(tree.positions.get(&fen_e5), Some(&1));
    }

    #[test]
    fn test_transposition() {
        let pgn = vec![
            "[White \"Magnus\"]\n[Black \"Doria\"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bb5 *".to_string(),
            "[White \"Magnus\"]\n[Black \"Doria\"]\n\n1. Nf3 Nc6 2. e4 e5 3. Bb5 *".to_string(),
        ];
        let tree = OpponentTree::build_from_pgn(pgn, "Magnus".to_string());
        // Même position atteinte par deux ordres — Magnus (blanc) joue Bb5 depuis ici
        let fen =
            "r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq -"
                .to_string();
        assert_eq!(tree.positions.get(&fen), Some(&2));
    }
}
