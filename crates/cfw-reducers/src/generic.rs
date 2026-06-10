use crate::{Reducer, Reduction};

pub struct GenericReducer;

impl Reducer for GenericReducer {
    fn name(&self) -> &'static str {
        "generic"
    }

    fn reduce(&self, input: &str) -> Reduction {
        const HEAD_LINES: usize = 80;
        const TAIL_LINES: usize = 40;

        let lines: Vec<&str> = input.lines().collect();
        if lines.len() <= HEAD_LINES + TAIL_LINES {
            return Reduction {
                reducer: self.name().to_string(),
                output: input.to_string(),
                omitted: false,
                notes: vec![],
            };
        }

        let mut output = String::new();
        for line in lines.iter().take(HEAD_LINES) {
            output.push_str(line);
            output.push('\n');
        }
        output.push_str(&format!(
            "\n[context-firewall: omitted {} middle lines]\n\n",
            lines.len() - HEAD_LINES - TAIL_LINES
        ));
        for line in lines.iter().skip(lines.len() - TAIL_LINES) {
            output.push_str(line);
            output.push('\n');
        }

        Reduction {
            reducer: self.name().to_string(),
            output,
            omitted: true,
            notes: vec!["kept first 80 and last 40 lines".to_string()],
        }
    }
}
