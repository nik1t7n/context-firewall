pub mod generic;
pub mod git;
pub mod test_output;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reduction {
    pub reducer: String,
    pub output: String,
    pub omitted: bool,
    pub notes: Vec<String>,
}

pub trait Reducer {
    fn name(&self) -> &'static str;
    fn reduce(&self, input: &str) -> Reduction;
}

pub fn reduce(kind: &str, input: &str) -> Reduction {
    match kind {
        "git" | "git-output" | "git_output" => git::GitReducer.reduce(input),
        "test-output" | "test_output" => test_output::TestOutputReducer.reduce(input),
        _ => generic::GenericReducer.reduce(input),
    }
}
