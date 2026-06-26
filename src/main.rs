mod config;
mod data;
mod repertoire;
mod pipeline;
mod engine;
use clap::Parser;
use config::Config;
use std::path::Path;
use std::path::PathBuf;

use crate::config::Profile;
#[derive(Parser)]
#[command(name = "chess-prep", about = "Outil de préparation aux échecs")]
struct Cli {
    /// Nom d'utilisateur Lichess de l'adversaire
    #[arg(long)]
    opponent: String,

    /// Fichier PGN de préparation de l'utilisateur
    #[arg(long)]
    user_pgn: PathBuf,

    #[arg(long, default_value = "standard")]
    profile: Profile,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let mut config = Config::from_profile(cli.profile);
    if Path::new("chess-prep.toml").exists() {
        config = Config::load(Path::new("chess-prep.toml"));
    }

    println!("1. Récupération des parties de l'adversaire ({}) depuis Lichess...", cli.opponent);
    let opponent_games = match data::lichess::fetch_games(&cli.opponent, config.opponent_games_limit, Some("blitz")) {
        Ok(games) => {
            println!("   -> {} parties récupérées.", games.len());
            games
        }
        Err(e) => {
            eprintln!("Erreur lors de la récupération des parties : {}", e);
            std::process::exit(1);
        }
    };

    println!("2. Chargement du fichier PGN de l'utilisateur ({:?})...", cli.user_pgn);
    let user_games = match data::pgn_import::load_pgn_file(&cli.user_pgn) {
        Ok(games) => {
            println!("   -> {} parties chargées.", games.len());
            games
        }
        Err(e) => {
            eprintln!("Erreur lors du chargement du fichier PGN : {}", e);
            std::process::exit(1);
        }
    };

    println!("3. Construction de l'OpponentTree...");
    let opponent_tree = repertoire::tree::OpponentTree::build_from_pgn(opponent_games, cli.opponent.clone());

    println!("4. Calcul des positions atteignables via le répertoire de l'utilisateur...");
    let reachability_engine = repertoire::reachability::ReachabilityEngine::new(config.max_search_depth, 0.01);
    let reachable_positions = reachability_engine.compute(user_games, &opponent_tree, config.min_novelty_ply);
    println!("   -> {} positions uniques atteignables trouvées.", reachable_positions.positions.len());

    println!("5. Calcul des scores de nouveauté (Profiler)...");
    let profiler = pipeline::profiler::Profiler::new(
        config.novelty_threshold,
        config.min_novelty_ply,
        config.min_parent_count,
    );
    let profiled_positions = profiler.profile(&reachable_positions.positions, &reachable_positions.parents, &opponent_tree);
    println!("   -> {} positions retenues sous le seuil de nouveauté.", profiled_positions.len());

    println!("6. Analyse des positions filtrées avec Stockfish...");
    let analyzer = pipeline::analyzer::Analyzer::new(config.clone());
    let analyzed_positions = analyzer.analyze(&profiled_positions)?;
    println!("   -> {} positions validées par l'analyse.", analyzed_positions.len());

    println!("7. Calcul du score final (Scoring)...");
    use crate::pipeline::scorer::Scorer;
    let scorer = Scorer::new(config.scoring.weights.clone());
    let scored_lines = scorer.score(&analyzed_positions);
    
    println!("   -> Top {} positions candidates recommandées :", config.max_candidates);
    for (i, line) in scored_lines.iter().take(config.max_candidates as usize).enumerate() {
        println!("      {}. Score Global: {:.2} (Nouveauté: {:.2}, Complexité: {:.2})", 
            i + 1, line.final_score, line.novelty_score, line.complexity_score);
        
        let mut path_moves = Vec::new();
        let mut current_fen = line.fen.clone();
        while let Some((parent_fen, san_move)) = reachable_positions.parents.get(&current_fen) {
            path_moves.push(san_move.clone());
            current_fen = parent_fen.clone();
        }
        path_moves.reverse();

        let path_str = if path_moves.is_empty() {
            "Position de départ".to_string()
        } else {
            let pv_san = pipeline::annotator::uci_pv_to_san(&line.fen, &line.pv);
            if !pv_san.is_empty() {
                let mut all = path_moves.clone();
                all.push(pv_san[0].clone());
                format_moves_as_pgn(&all)
            } else {
                format_moves_as_pgn(&path_moves)
            }
        };

        println!("         Ligne: {}", path_str);
        println!("         FEN: {}", line.fen);
        println!("         Coup suggéré: {}", line.best_move);
    }

    println!("8. Génération du fichier PGN de préparation...");
    use crate::pipeline::annotator::Annotator;
    let raw_user_pgn = std::fs::read_to_string(&cli.user_pgn)?;
    let annotator = pipeline::annotator::BasicAnnotator {
        opponent: cli.opponent.clone(),
        parents: reachable_positions.parents,
    };
    let annotated_pgn = annotator.annotate(&raw_user_pgn, &scored_lines)?;

    println!("9. Exportation du PGN de préparation...");
    let output_file = config.output_dir.join(format!("{}_prep.pgn", cli.opponent));
    std::fs::create_dir_all(&config.output_dir)?;
    std::fs::write(&output_file, annotated_pgn)?;
    println!("   -> Préparation exportée avec succès dans : {:?}", output_file);

    Ok(())
}

fn format_moves_as_pgn(moves: &[String]) -> String {
    let mut pgn = String::new();
    for (i, mv) in moves.iter().enumerate() {
        if i % 2 == 0 {
            pgn.push_str(&format!("{}. {} ", i / 2 + 1, mv));
        } else {
            pgn.push_str(&format!("{} ", mv));
        }
    }
    pgn.trim_end().to_string()
}
