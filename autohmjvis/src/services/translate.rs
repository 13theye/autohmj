// src/services/translate.rs
//
// uses the Rust Translators crate to access Google Translate.
//
//
use deeplx::{Config, DeepLX};

#[derive(Clone)]
pub struct Translate {
    pub translator: DeepLX,
}

impl Default for Translate {
    fn default() -> Self {
        let deeplx_trans = DeepLX::new(Config {
            ..Default::default()
        });
        Self {
            translator: deeplx_trans,
        }
    }
}

impl Translate {
    async fn get_translation(
        &self,
        input: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Option<String> {
        let result = self
            .translator
            .translate(source_lang, target_lang, input, None, None)
            .await;

        match result {
            Ok(res) => {
                println!("Target: {}, Result: {}", target_lang, res.data);
                Some(res.data.trim_end().to_owned())
            }
            Err(e) => {
                eprintln!("Error in Translate: {}", e);
                None
            }
        }
    }

    pub async fn to_english(&self, input: &str) -> Option<String> {
        self.get_translation(input, "auto", "en").await
    }

    pub async fn to_korean(&self, input: &str) -> Option<String> {
        self.get_translation(input, "auto", "ko").await
    }
}
