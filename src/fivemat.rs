//! Indent-aware indenting

use std::fmt::Write;

/// A wrapper for a std::fmt::Write impl that automatically indents.
pub struct Fivemat<'a> {
    indent_text: String,
    indent: usize,
    indent_pending: bool,
    out: &'a mut dyn Write,
}

impl<'a> Fivemat<'a> {
    pub fn new(out: &'a mut dyn Write, indent_text: impl Into<String>) -> Self {
        Fivemat {
            indent_text: indent_text.into(),
            indent: 0,
            indent_pending: true,
            out,
        }
    }
    pub fn add_indent(&mut self, count: usize) {
        self.indent += count;
    }
    pub fn sub_indent(&mut self, count: usize) {
        self.indent -= count;
    }
    pub fn offset_indent(&mut self, count: isize) {
        self.indent = (self.indent as isize + count) as usize;
    }
    pub fn ensure_indent(&mut self) -> std::fmt::Result {
        if !self.indent_pending {
            return Ok(());
        }
        for _ in 0..self.indent {
            write!(&mut self.out, "{}", self.indent_text)?;
        }
        self.indent_pending = false;
        Ok(())
    }
    pub fn newline(&mut self) -> std::fmt::Result {
        self.indent_pending = true;
        write!(&mut self.out, "\n")
    }
}

impl<'a> std::fmt::Write for Fivemat<'a> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let mut multiline = false;
        let ends_with_newline = s.ends_with('\n') || s.ends_with("\r\n") || s.ends_with("\n\r");
        for line in s.lines() {
            if multiline {
                self.newline()?;
            }
            multiline = true;
            if line.len() > 0 {
                self.ensure_indent()?;
                write!(&mut self.out, "{}", line)?;
            }
        }
        if ends_with_newline {
            self.newline()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn fivemat_basic() -> std::fmt::Result {
        let mut out = String::new();
        let mut f = Fivemat::new(&mut out, "    ");
        writeln!(&mut f)?;
        {
            writeln!(&mut f, "struct MyStruct {{")?;
            f.add_indent(1);
            {
                write!(&mut f, "field1: ")?;
                write!(&mut f, "bool")?;
                writeln!(&mut f, ",")?;
            }
            {
                write!(&mut f, "field2: ")?;
                writeln!(&mut f, "{} {{", "Inner")?;
                f.add_indent(1);
                {
                    writeln!(&mut f, "{}: {},", "x", "f64")?;
                    writeln!(&mut f, "y: f32,")?;
                }
                f.sub_indent(1);
                writeln!(&mut f, "}},")?;
            }
            writeln!(
                &mut f,
                r#"field3: MyThing {{
    a: i32,
    b: i64,
}},"#
            )?;
            f.sub_indent(1);
            writeln!(&mut f, "}}")?;
        }
        writeln!(&mut f)?;

        {
            writeln!(&mut f, "fn my_func() {{")?;
            f.add_indent(1);
            {
                writeln!(&mut f, "let x = 0;")?;
                writeln!(&mut f)?;
                writeln!(&mut f, "let y = 5;")?;
                writeln!(&mut f, "\n")?;
                writeln!(&mut f, "let z = 10;")?;
                write!(&mut f, "let w = 100;\nlet q = ")?;
                write!(&mut f, "20")?;
                writeln!(&mut f, ";")?;
            }
            f.sub_indent(1);
            writeln!(&mut f, "}}")?;
        }

        assert_eq!(
            out,
            r#"
struct MyStruct {
    field1: bool,
    field2: Inner {
        x: f64,
        y: f32,
    },
    field3: MyThing {
        a: i32,
        b: i64,
    },
}

fn my_func() {
    let x = 0;

    let y = 5;


    let z = 10;
    let w = 100;
    let q = 20;
}
"#
        );

        Ok(())
    }
}
