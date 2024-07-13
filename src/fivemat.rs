//! Indent-aware indenting
//!
//! FIXME: it would be *great* if this had APIs for like
//! "I am starting/ending a function" and "I am starting/ending a variable"
//! so that it could implicitly constructs spans for them. In theory
//! this would unlock the ability for the test harness' error-reporting
//! facilities to say "hey these two impls disagreed on the values of var 1 field 3"
//! **and actually show the Rust/C sourcecode that corresponds to**.

use std::fmt::Write;

/// A wrapper for a std::fmt::Write impl that automatically indents.
pub struct Fivemat<'a> {
    indent_text: String,
    indent: usize,
    indent_pending: bool,
    out: &'a mut dyn Write,
}

pub struct FivematIndent<'a, 'b> {
    inner: &'b mut Fivemat<'a>,
    count: usize,
}
impl Drop for FivematIndent<'_, '_> {
    fn drop(&mut self) {
        self.inner.sub_indent(self.count);
    }
}
impl<'a, 'b> std::ops::Deref for FivematIndent<'a, 'b> {
    type Target = Fivemat<'a>;
    fn deref(&self) -> &Fivemat<'a> {
        self.inner
    }
}
impl<'a, 'b> std::ops::DerefMut for FivematIndent<'a, 'b> {
    fn deref_mut(&mut self) -> &mut Fivemat<'a> {
        self.inner
    }
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
    pub fn indent<'b>(&'b mut self) -> FivematIndent<'a, 'b> {
        self.indent_many(1)
    }
    pub fn indent_many<'b>(&'b mut self, count: usize) -> FivematIndent<'a, 'b> {
        self.add_indent(count);
        FivematIndent { inner: self, count }
    }

    pub fn add_indent(&mut self, count: usize) {
        self.indent += count;
    }
    pub fn sub_indent(&mut self, count: usize) {
        self.indent -= count;
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
        writeln!(&mut self.out)
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
            if !line.is_empty() {
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
        let ident_inner = "Inner";
        let ident_x = "x";
        let ty_f64 = "f64";
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
                writeln!(&mut f, "{} {{", ident_inner)?;
                f.add_indent(1);
                {
                    writeln!(&mut f, "{}: {},", ident_x, ty_f64)?;
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
