use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio, Child};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Evaluation {
    Cp(i32),
    Mate(i32),
}

impl Evaluation {
    pub fn to_f64(&self) -> f64 {
        match self {
            Evaluation::Cp(cp) => *cp as f64 / 100.0,
            Evaluation::Mate(m) => {
                if *m > 0 {
                    100.0 - *m as f64
                } else {
                    -100.0 - *m as f64
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ScoredMove {
    pub uci: String,
    pub eval: Evaluation,
    pub pv: Vec<String>,
}

pub struct StockfishClient {
    child: Child,
    stdin: std::process::ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
}

impl StockfishClient {
    pub fn new(path: &str) -> Result<Self, std::io::Error> {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to open stdin")
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to open stdout")
        })?;

        let mut client = StockfishClient {
            child,
            stdin,
            reader: BufReader::new(stdout),
        };

        client.init()?;
        Ok(client)
    }

    fn init(&mut self) -> Result<(), std::io::Error> {
        self.stdin.write_all(b"uci\n")?;
        self.stdin.flush()?;

        let mut line = String::new();
        loop {
            line.clear();
            self.reader.read_line(&mut line)?;
            if line.trim() == "uciok" {
                break;
            }
        }

        self.stdin.write_all(b"setoption name MultiPV value 3\n")?;
        self.stdin.write_all(b"isready\n")?;
        self.stdin.flush()?;

        loop {
            line.clear();
            self.reader.read_line(&mut line)?;
            if line.trim() == "readyok" {
                break;
            }
        }
        Ok(())
    }

    /// Runs Stockfish analysis on the given FEN up to the requested depth.
    /// Returns the top moves and their evaluations (sorted by preference).
    pub fn analyze_position(&mut self, fen: &str, depth: u8) -> Result<Vec<ScoredMove>, std::io::Error> {
        self.stdin.write_all(format!("position fen {}\n", fen).as_bytes())?;
        self.stdin.write_all(format!("go depth {}\n", depth).as_bytes())?;
        self.stdin.flush()?;

        let mut line = String::new();
        let mut moves_map = std::collections::HashMap::new();

        loop {
            line.clear();
            self.reader.read_line(&mut line)?;
            let trimmed = line.trim();
            if trimmed.starts_with("bestmove") {
                break;
            }
            if trimmed.starts_with("info") {
                if let Some((mpv, scored_move)) = parse_info_line(trimmed) {
                    moves_map.insert(mpv, scored_move);
                }
            }
        }

        let mut result: Vec<(u32, ScoredMove)> = moves_map.into_iter().collect();
        result.sort_by_key(|(mpv, _)| *mpv);

        Ok(result.into_iter().map(|(_, sm)| sm).collect())
    }
}

impl Drop for StockfishClient {
    fn drop(&mut self) {
        let _ = self.stdin.write_all(b"quit\n");
        let _ = self.stdin.flush();
        let _ = self.child.kill();
    }
}

/// Helper function to parse UCI info lines containing multiPV scores.
fn parse_info_line(line: &str) -> Option<(u32, ScoredMove)> {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    let mut multipv = None;
    let mut score_type = None;
    let mut score_val = None;
    let mut pv = Vec::new();

    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "multipv" => {
                if i + 1 < tokens.len() {
                    multipv = tokens[i + 1].parse::<u32>().ok();
                    i += 2;
                } else { i += 1; }
            }
            "score" => {
                if i + 2 < tokens.len() {
                    score_type = Some(tokens[i + 1]);
                    score_val = tokens[i + 2].parse::<i32>().ok();
                    i += 3;
                } else { i += 1; }
            }
            "pv" => {
                for token in &tokens[i + 1..] {
                    pv.push(token.to_string());
                }
                break;
            }
            _ => {
                i += 1;
            }
        }
    }

    if let (Some(mpv), Some(stype), Some(sval)) = (multipv, score_type, score_val) {
        if pv.is_empty() {
            return None;
        }
        let uci = pv[0].clone();
        let eval = match stype {
            "cp" => Evaluation::Cp(sval),
            "mate" => Evaluation::Mate(sval),
            _ => return None,
        };
        Some((mpv, ScoredMove { uci, eval, pv }))
    } else {
        None
    }
}

pub fn find_stockfish_binary() -> String {
    if Command::new("stockfish")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
    {
        return "stockfish".to_string();
    }
    for path in &["/opt/homebrew/bin/stockfish", "/usr/local/bin/stockfish"] {
        if std::path::Path::new(path).exists() {
            return path.to_string();
        }
    }
    "stockfish".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_and_run_stockfish() {
        let path = find_stockfish_binary();
        // Skip test if stockfish is not installed anywhere
        if path == "stockfish" && !Command::new("stockfish").arg("--version").status().is_ok() {
            return;
        }

        let mut client = StockfishClient::new(&path).unwrap();
        
        // Start position
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq -";
        let moves = client.analyze_position(fen, 8).unwrap();
        
        assert!(!moves.is_empty());
        assert!(moves.len() <= 3);
        
        // First move evaluation should be valid
        let best = &moves[0];
        assert!(best.uci.len() >= 4);
        assert!(!best.pv.is_empty());
        assert_eq!(best.pv[0], best.uci);
        match best.eval {
            Evaluation::Cp(cp) => {
                // Usually evaluation of start position is around 0 to 50 centipawns
                assert!(cp.abs() < 300);
            }
            Evaluation::Mate(_) => {
                panic!("Start position cannot be mate");
            }
        }
    }

    #[test]
    fn test_parse_info_line() {
        let line = "info depth 8 seldepth 8 multipv 1 score cp 24 nodes 248 nps 248000 time 1 pv e2e4 e7e5";
        let res = parse_info_line(line).unwrap();
        assert_eq!(res.0, 1);
        assert_eq!(res.1.uci, "e2e4");
        assert_eq!(res.1.eval, Evaluation::Cp(24));
        assert_eq!(res.1.pv, vec!["e2e4".to_string(), "e7e5".to_string()]);

        let line_mate = "info depth 5 multipv 2 score mate -3 pv d2d4 d7d5";
        let res_mate = parse_info_line(line_mate).unwrap();
        assert_eq!(res_mate.0, 2);
        assert_eq!(res_mate.1.uci, "d2d4");
        assert_eq!(res_mate.1.eval, Evaluation::Mate(-3));
        assert_eq!(res_mate.1.pv, vec!["d2d4".to_string(), "d7d5".to_string()]);
    }
}
