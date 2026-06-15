pub mod browser;
pub mod generic;
pub mod git;
pub mod json;
pub mod log;
pub mod outline;
pub mod search;
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
        "browser" | "browser-snapshot" | "browser_snapshot" | "aria-snapshot" | "aria_snapshot" => {
            browser::BrowserSnapshotReducer.reduce(input)
        }
        "git" | "git-output" | "git_output" => git::GitReducer.reduce(input),
        "json" | "json-output" | "json_output" => json::JsonReducer.reduce(input),
        "log" | "log-output" | "log_output" => log::LogReducer.reduce(input),
        "outline" | "file-outline" | "file_outline" => outline::OutlineReducer.reduce(input),
        "search" | "search-output" | "search_output" => search::SearchReducer.reduce(input),
        "test-output" | "test_output" => test_output::TestOutputReducer.reduce(input),
        _ => generic::GenericReducer.reduce(input),
    }
}
