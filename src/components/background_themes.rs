// src/components/background_themes.rs
use dioxus::prelude::*;

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
                url: "https://raw.githubusercontent.com/hogyzen12/unruggable-app/refs/heads/main/assets/icons/bg.png".to_string(),
                description: "Minimalist special edition for the Solana mobile hackathon.".to_string(),
            },    
            BackgroundTheme {
                name: "Seeker x Unruggable".to_string(),
                url: "https://raw.githubusercontent.com/hogyzen12/unruggable-app/refs/heads/main/public/bg.jpeg".to_string(),
                description: "Special edition for the launch of Solana Seeker.".to_string(),
            },
            BackgroundTheme {
                name: "Pudgy x Unruggable".to_string(),
                url: "https://media.discordapp.net/attachments/1143657925009756233/1398455227853701222/pudgy.jpeg?ex=68856c6c&is=68841aec&hm=0d8f2d46a29e99601ab39c1d18ffca7756954afa5c88a582515d58e7d0ec363d&=&format=webp&width=856&height=856&fit=crop".to_string(),
                description: "u know what is. pudgies!!".to_string(),
            },
            BackgroundTheme {
                name: "STUK x Unruggable".to_string(),
                url: "https://media.discordapp.net/attachments/1143657925009756233/1308495813898666094/8BC8BCA3-2447-4660-B26E-6BAEB549962A.jpg?ex=68851b30&is=6883c9b0&hm=b70ae599c08c87e965b426900134ee5e21d6d348a738a744cdc1348b1bf37e4f&=&format=webp&width=605&height=856&fit=crop".to_string(),
                description: "Superteam is a cheatcode".to_string(),
            },
            BackgroundTheme {
                name: "Unruggable".to_string(),
                url: "https://media.discordapp.net/attachments/1289308318589911040/1299961842638131200/unrgble_update_.jpg?ex=68850a8f&is=6883b90f&hm=aba6384f40be1cddf4f778a86464a5aded9b32486d96b0dd3d02452c51997e1b&=&format=webp&width=856&height=856&fit=crop".to_string(),
                description: "Let him cook".to_string(),
            },            
        ]
    }
}