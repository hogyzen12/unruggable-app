// src/components/background_themes.rs
use dioxus::prelude::*;

//const LOCAL_BG: Asset = asset!("/assets/backgrounds/grey.webp");

#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundTheme {
    pub name: String,
    pub url: String,
    pub description: String,
}

impl BackgroundTheme {
    pub fn get_presets() -> Vec<BackgroundTheme> {
        vec![
            BackgroundTheme {
                name: "Solana Summer".to_string(),
                url: "https://raw.githubusercontent.com/hogyzen12/unruggable-app/refs/heads/main/assets/backgrounds/bg.png".to_string(),
                description: "Minimalist special edition for the Solana mobile hackathon.".to_string(),
            },    
            BackgroundTheme {
                name: "Seeker x Unruggable".to_string(),
                url: "https://raw.githubusercontent.com/hogyzen12/unruggable-app/refs/heads/main/assets/backgrounds/bg.jpeg".to_string(),
                description: "Special edition for the launch of Solana Seeker.".to_string(),
            },
            BackgroundTheme {
                name: "Two Tap Staking".to_string(),
                url: "https://raw.githubusercontent.com/hogyzen12/unruggable-app/refs/heads/main/assets/backgrounds/stake.webp".to_string(),
                description: "Stake with us".to_string(),
            }, 
            BackgroundTheme {
                name: "Jito x Unruggable".to_string(),
                url: "https://raw.githubusercontent.com/hogyzen12/unruggable-app/refs/heads/main/assets/backgrounds/fastaf.webp".to_string(),
                description: "txs fast afff".to_string(),
            }, 
            //BackgroundTheme {
            //    name: "STUK x Unruggable".to_string(),
            //    url: LOCAL_BG.to_string(),
            //    description: "Superteam is a cheatcode".to_string(),
            //},           
        ]
    }
}