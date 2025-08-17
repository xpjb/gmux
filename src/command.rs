#[derive(Debug, Clone, Copy)]
pub enum Command {
    Dmenu,
    Terminal,
    Screenshot,
}

impl Command {
    pub fn str(&self) -> &str {
        match self {
            Command::Dmenu => "dmenu_run -m 0 -fn 'monospace:size=16' -nb '#222222' -nf '#bbbbbb' -sb '#005577' -sf '#eeeeee'",
            Command::Terminal => "alacritty",
            Command::Screenshot => "/home/pat/repo/scripts/screenshot",
        }
    }
}