use maud::{Render, html};

pub struct ChainOption {
    pub label: String,
    pub value: String,
}

impl Render for ChainOption {
    fn render(&self) -> maud::Markup {
        html! {
            option value=(self.value) {
                (self.label)
            }
        }
    }
}

pub struct ChainSelector {
    pub id: String,
    pub hx_target: String,
    pub tauri_invoke: String,
    pub chain_options: Vec<ChainOption>,
}

impl ChainSelector {
    const NAME: &str = "selectedChain";
}

impl Render for ChainSelector {
    fn render(&self) -> maud::Markup {
        html! {
            label for=(self.id) .label {"Select your favourite chain"}
            select .select  id=(self.id) name=(Self::NAME)
            "hx-target"=(self.hx_target) "tauri-invoke"=(self.tauri_invoke) {
               @for chain in &self.chain_options {
                   (chain)
               }
            }
        }
    }
}

pub struct Nodes {
    pub id: String,
    pub node_id: u64,
    pub node_url: String,
}
