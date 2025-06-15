# KDLScript function bodies

The kdl-script compiler does technically support function bodies that it can run and evaluate.
This is *completely* useless to kdl-script-as-used-by-abi-cafe, and is entirely just a fun shitpost.

The evaluator has not at all kept up with the type system, so it can only handle some really simply stuff.
You can run the `examples/simple.kdl`. All the other examples will just dump type information and decl order
as they don't define `main`.

```text
> cargo run examples/simple.kdl

{
  y: 22
  x: 11
}
33
```

Is executing the following kdl document:


```kdl
struct "Point" {
    x "f64"
    y "f64"
}

fn "main" {
    outputs { _ "f64"; }

    let "pt1" "Point" {
        x 1.0
        y 2.0
    }
    let "pt2" "Point" {
        x 10.0
        y 20.0
    }

    let "sum" "add:" "pt1" "pt2"
    print "sum"

    return "+:" "sum.x" "sum.y"
}

fn "add" {
    inputs { a "Point"; b "Point"; }
    outputs { _ "Point"; }

    return "Point" {
        x "+:" "a.x" "b.x"
        y "+:" "a.y" "b.y"
    }
}
```


# Why Did You Make KDL Documents Executable???

To spite parsers.

Ok more seriously because I needed the parser and type-system for abi-cafe but it's a ton of work so I'm self-motivated by wrapping it in the guise of a scripting language because it's funny and I could make more incremental progress. This in fact worked, because as of the publishing of this book, abi-cafe was rewritten to use kdl-script!
