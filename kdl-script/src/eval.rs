use std::{collections::HashMap, sync::Arc};

use miette::NamedSource;

use crate::{
    parse::{Expr, FuncDecl, Literal, ParsedProgram, Stmt},
    spanned::Spanned,
    Result,
};

#[derive(Debug, Clone)]
enum Val {
    Struct(HashMap<String, Val>),
    Int(i64),
    Float(f64),
    Bool(bool),
}

pub fn eval_kdl_script(_src: &Arc<NamedSource>, program: &ParsedProgram) -> Result<i64> {
    let main = lookup_func(program, "main");

    let val = eval_call(program, main, HashMap::default());

    match val {
        Val::Int(val) => Ok(val),
        Val::Float(val) => Ok(val as i64),
        Val::Bool(val) => Ok(val as i64),
        Val::Struct(_) => {
            unreachable!("main returned struct!")
        }
    }
}

fn eval_call(program: &ParsedProgram, func: &FuncDecl, mut vars: HashMap<String, Val>) -> Val {
    for stmt in &func.body {
        match stmt {
            Stmt::Let(stmt) => {
                let temp = eval_expr(program, &stmt.expr, &vars);
                if let Some(var) = &stmt.var {
                    vars.insert(var.to_string(), temp);
                }
            }
            Stmt::Return(stmt) => {
                let temp = eval_expr(program, &stmt.expr, &vars);
                return temp;
            }
            Stmt::Print(stmt) => {
                let temp = eval_expr(program, &stmt.expr, &vars);
                print_val(&temp);
            }
        }
    }
    unreachable!("function didn't return!");
}

fn eval_expr(program: &ParsedProgram, expr: &Spanned<Expr>, vars: &HashMap<String, Val>) -> Val {
    match &**expr {
        Expr::Call(expr) => {
            let func = lookup_func(program, &expr.func);
            assert_eq!(
                func.inputs.len(),
                expr.args.len(),
                "function {} had wrong number of args",
                &**expr.func
            );
            let input = func
                .inputs
                .iter()
                .zip(expr.args.iter())
                .map(|(var, expr)| {
                    let val = eval_expr(program, expr, vars);
                    let var = var.name.as_ref().unwrap().to_string();
                    (var, val)
                })
                .collect();

            match func.name.as_str() {
                "+" => eval_add(input),
                _ => eval_call(program, func, input),
            }
        }
        Expr::Path(expr) => {
            let mut sub_val = vars
                .get(&**expr.var)
                .unwrap_or_else(|| panic!("couldn't find var {}", &**expr.var));
            for field in &expr.path {
                if let Val::Struct(val) = sub_val {
                    sub_val = val
                        .get(field.as_str())
                        .unwrap_or_else(|| panic!("couldn't find field {}", &**field));
                } else {
                    panic!("tried to get .{} on primitive", &**field);
                }
            }
            sub_val.clone()
        }
        Expr::Ctor(expr) => {
            let fields = expr
                .vals
                .iter()
                .map(|stmt| {
                    let val = eval_expr(program, &stmt.expr, vars);
                    let var = stmt.var.as_ref().unwrap().to_string();
                    (var, val)
                })
                .collect();
            Val::Struct(fields)
        }
        Expr::Literal(expr) => match expr.val {
            Literal::Float(val) => Val::Float(val),
            Literal::Int(val) => Val::Int(val),
            Literal::Bool(val) => Val::Bool(val),
        },
    }
}

fn print_val(val: &Val) {
    match val {
        Val::Struct(vals) => {
            println!("{{");
            for (k, v) in vals {
                print!("  {k}: ");
                print_val(v);
            }
            println!("}}");
        }
        Val::Int(val) => println!("{val}"),
        Val::Float(val) => println!("{val}"),
        Val::Bool(val) => println!("{val}"),
    }
}

fn eval_add(input: HashMap<String, Val>) -> Val {
    let lhs = input.get("lhs").unwrap();
    let rhs = input.get("rhs").unwrap();
    match (lhs, rhs) {
        (Val::Int(lhs), Val::Int(rhs)) => Val::Int(lhs + rhs),
        (Val::Float(lhs), Val::Float(rhs)) => Val::Float(lhs + rhs),
        _ => {
            panic!("unsupported addition pair");
        }
    }
}

fn lookup_func<'a>(program: &'a ParsedProgram, func_name: &str) -> &'a FuncDecl {
    let func = program
        .funcs
        .iter()
        .find(|(name, _f)| name.as_str() == func_name);
    if func.is_none() {
        panic!("couldn't find {func_name} function");
    }
    func.unwrap().1
}
