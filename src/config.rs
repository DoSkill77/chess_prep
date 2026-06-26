use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EvalBounds {
    pub white_min: f64,
    pub black_min: f64,
}

impl Default for EvalBounds {
    fn default() -> Self {
        EvalBounds {
            white_min: 0.0,
            black_min: -0.5,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ScoringWeights {
    pub novelty: f64,
    pub complexity: f64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        ScoringWeights {
            novelty: 0.60,
            complexity: 0.40,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Scoring {
    pub weights: ScoringWeights,
}

impl Default for Scoring {
    fn default() -> Self {
        Scoring {
            weights: ScoringWeights::default(),
        }
    }
}

fn default_min_novelty_ply() -> u32 { 6 }
fn default_min_parent_count() -> u32 { 3 }

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Config {
    pub stockfish_depth_filter: u8,
    pub stockfish_depth_validate: u8,
    pub max_search_depth: u8,
    pub novelty_threshold: u32,
    pub opponent_games_limit: u32,
    pub max_candidates: u32,
    pub output_dir: PathBuf,
    pub analytics_enabled: bool,
    #[serde(default)]
    pub eval_bounds: EvalBounds,
    #[serde(default)]
    pub scoring: Scoring,
    #[serde(default = "default_min_novelty_ply")]
    pub min_novelty_ply: u32,
    #[serde(default = "default_min_parent_count")]
    pub min_parent_count: u32,
}

#[derive(Clone)]
pub enum Profile {
    Quick,
    Standard,
    Deep,
}
impl FromStr for Profile {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "quick" => Ok(Profile::Quick),
            "standard" => Ok(Profile::Standard),
            "deep" => Ok(Profile::Deep),
            other => Err(format!("Profil inconnu : {}", other)),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Config {
            stockfish_depth_filter: 15,
            stockfish_depth_validate: 22,
            max_search_depth: 15,
            novelty_threshold: 5,
            opponent_games_limit: 500,
            max_candidates: 20,
            output_dir: "./output".into(),
            analytics_enabled: false,
            eval_bounds: EvalBounds::default(),
            scoring: Scoring::default(),
            min_novelty_ply: 6,
            min_parent_count: 3,
        }
    }
}
impl Config {
    pub fn from_profile(profile: Profile) -> Self {
        match profile {
            Profile::Quick => Config {
                stockfish_depth_filter: 10,
                stockfish_depth_validate: 15,
                max_search_depth: 10,
                novelty_threshold: 10,
                opponent_games_limit: 100,
                max_candidates: 10,
                output_dir: "./output".into(),
                analytics_enabled: false,
                eval_bounds: EvalBounds {
                    white_min: 0.0,
                    black_min: -0.5,
                },
                scoring: Scoring {
                    weights: ScoringWeights {
                        novelty: 0.60,
                        complexity: 0.40,
                    },
                },
                min_novelty_ply: 6,
                min_parent_count: 3,
            },
            Profile::Standard => Config::default(),
            Profile::Deep => Config {
                stockfish_depth_filter: 18,
                stockfish_depth_validate: 28,
                max_search_depth: 20,
                novelty_threshold: 3,
                opponent_games_limit: 1000,
                max_candidates: 30,
                output_dir: "./output".into(),
                analytics_enabled: false,
                eval_bounds: EvalBounds {
                    white_min: 0.0,
                    black_min: -0.5,
                },
                scoring: Scoring {
                    weights: ScoringWeights {
                        novelty: 0.60,
                        complexity: 0.40,
                    },
                },
                min_novelty_ply: 8,
                min_parent_count: 5,
            },
        }
    }
    pub fn load(path: &Path) -> Self {
        let content = std::fs::read_to_string(path)
            .expect("Impossible de lire le fichier de config");
        toml::from_str(&content).expect("Fichier de config invalide")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default() {
        let config = Config::default();
        assert_eq!(config.stockfish_depth_filter, 15);
        assert_eq!(config.novelty_threshold, 5);
        assert_eq!(config.opponent_games_limit, 500);
        assert_eq!(config.min_novelty_ply, 6);
        assert_eq!(config.min_parent_count, 3);
    }

    #[test]
    fn test_from_profile_quick() {
        let config = Config::from_profile(Profile::Quick);
        assert_eq!(config.stockfish_depth_filter, 10);
        assert_eq!(config.opponent_games_limit, 100);
        assert_eq!(config.max_candidates, 10);
    }

    #[test]
    fn test_from_profile_deep() {
        let config = Config::from_profile(Profile::Deep);
        assert_eq!(config.stockfish_depth_filter, 18);
        assert_eq!(config.opponent_games_limit, 1000);
        assert_eq!(config.novelty_threshold, 3);
        assert_eq!(config.min_novelty_ply, 8);
    }

    #[test]
    fn test_load_from_toml() {
        let path = std::path::Path::new("/tmp/test_config.toml");
        std::fs::write(
            path,
            r#"
            stockfish_depth_filter = 12
            stockfish_depth_validate = 20
            max_search_depth = 12
            novelty_threshold = 7
            opponent_games_limit = 200
            max_candidates = 15
            output_dir = "./output"
            analytics_enabled = false
        "#,
        )
        .unwrap();
        let config = Config::load(path);
        assert_eq!(config.stockfish_depth_filter, 12);
        assert_eq!(config.novelty_threshold, 7);
        assert_eq!(config.min_novelty_ply, 6); // loads default value
    }
}
