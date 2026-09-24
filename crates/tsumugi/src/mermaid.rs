//! Mermaid flowchart rendering.

use crate::workflow::Workflow;
use std::fmt::Write;

impl<S: Send> Workflow<S> {
    /// Renders the workflow as a [Mermaid](https://mermaid.js.org) flowchart.
    ///
    /// Declared transitions are drawn as edges, and steps declared with
    /// [`terminal`](crate::WorkflowBuilder::terminal) are connected to an end
    /// node. Steps without declared transitions are drawn with a dashed border,
    /// since their successors are only known at runtime.
    ///
    /// The output can be pasted into Markdown on GitHub, GitLab and many other
    /// tools inside a ```` ```mermaid ```` code block.
    ///
    /// # Examples
    ///
    /// ```
    /// use tsumugi::prelude::*;
    ///
    /// let workflow = Workflow::builder()
    ///     .add_fn("validate", |_ctx| Ok(Next::step("save")))
    ///     .then(["save"])
    ///     .add_fn("save", |_ctx| Ok(Next::Done))
    ///     .terminal()
    ///     .build()
    ///     .expect("valid workflow");
    ///
    /// assert_eq!(
    ///     workflow.to_mermaid(),
    ///     "flowchart TD\n    \
    ///      __start((start))\n    \
    ///      __end((end))\n    \
    ///      s0[\"validate\"]\n    \
    ///      s1[\"save\"]\n    \
    ///      __start --> s0\n    \
    ///      s0 --> s1\n    \
    ///      s1 --> __end\n"
    /// );
    /// ```
    pub fn to_mermaid(&self) -> String {
        let mut out = String::from("flowchart TD\n");
        let has_terminal = self
            .steps
            .iter()
            .any(|entry| matches!(&entry.transitions, Some(t) if t.is_empty()));
        let has_dynamic = self.steps.iter().any(|entry| entry.transitions.is_none());

        // Writing to a String cannot fail, so the results are ignored.
        let _ = writeln!(out, "    __start((start))");
        if has_terminal {
            let _ = writeln!(out, "    __end((end))");
        }
        for (i, entry) in self.steps.iter().enumerate() {
            let _ = writeln!(out, "    s{}[\"{}\"]", i, escape(entry.name.as_str()));
        }

        let _ = writeln!(out, "    __start --> s{}", self.start);
        for (i, entry) in self.steps.iter().enumerate() {
            match &entry.transitions {
                Some(targets) if targets.is_empty() => {
                    let _ = writeln!(out, "    s{} --> __end", i);
                }
                Some(targets) => {
                    for target in targets {
                        if let Some(j) = self.index.get(target) {
                            let _ = writeln!(out, "    s{} --> s{}", i, j);
                        }
                    }
                }
                None => {}
            }
        }

        if has_dynamic {
            let _ = writeln!(out, "    classDef dynamic stroke-dasharray: 5 5");
            for (i, entry) in self.steps.iter().enumerate() {
                if entry.transitions.is_none() {
                    let _ = writeln!(out, "    class s{} dynamic", i);
                }
            }
        }

        out
    }
}

/// Escapes characters that would break a quoted Mermaid label.
fn escape(label: &str) -> String {
    let mut out = String::with_capacity(label.len());
    for c in label.chars() {
        match c {
            '"' => out.push_str("#quot;"),
            '<' => out.push_str("#lt;"),
            '>' => out.push_str("#gt;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsumugi_core::Next;

    #[test]
    fn test_escape() {
        assert_eq!(escape(r#"a "b" <c>"#), "a #quot;b#quot; #lt;c#gt;");
    }

    #[test]
    fn test_dynamic_steps_are_dashed() {
        let workflow = Workflow::builder()
            .add_fn("a", |_ctx| Ok(Next::step("b")))
            .then(["b"])
            .add_fn("b", |_ctx| Ok(Next::Done))
            .build()
            .expect("valid workflow");

        let mermaid = workflow.to_mermaid();
        assert!(mermaid.contains("    s0 --> s1\n"));
        assert!(mermaid.contains("    class s1 dynamic\n"));
        assert!(!mermaid.contains("class s0 dynamic"));
        assert!(!mermaid.contains("__end"));
    }
}
