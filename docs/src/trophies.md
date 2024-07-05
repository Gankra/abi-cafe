# Trophy Case

Has ABI Cafe helped you find/fix a bug in your compiler? We'd love to hear!

* [x64 linux clang and gcc disagree on __int128 pass-on-stack ABI](https://github.com/rust-lang/rust/issues/54341#issuecomment-1064729606)
  * [now fixed in both rustc and clang!](https://blog.rust-lang.org/2024/03/30/i128-layout-update.html)
  * We already knew clang and rustc disagreed [because clang does a manual alignment adjustment](https://reviews.llvm.org/D28990), but we didn't seem to fully understand that the clang adjustment is actually buggy and doesn't apply to the implicit push-to-stack when passing __int128 by-val. gcc always aligns the value, even when pushing to stack, so the two desync in this case.
  * This tool was written to investigate the clang-rustc issue, and helped establish that everyone agreed on the ABI on ARM64, where __int128 is essentially part of the *hardware's* ABI due to it showing up in the layout for saving/restoring SIMD register state. As a result, [rust's libc crate now exposes typedefs for __int128 on those platforms](https://github.com/rust-lang/libc/pull/2719)

* [rustc_codegen_cranelift ICE on passing 11 bools by-val](https://github.com/bjorn3/rustc_codegen_cranelift/issues/1234)
