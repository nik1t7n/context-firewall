#[derive(Debug, Clone, Copy)]
pub struct TokenEstimate {
    pub tokens: i64,
    pub confidence: EstimateConfidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstimateConfidence {
    Low,
    Medium,
}

pub fn estimate_tokens(text: &str) -> TokenEstimate {
    let chars = text.chars().count() as i64;
    let tokens = (chars.max(1) + 3) / 4;
    TokenEstimate {
        tokens,
        confidence: EstimateConfidence::Low,
    }
}
