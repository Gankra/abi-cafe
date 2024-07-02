# abi-cafe 🧩☕️❤️

Not sure if your compilers have matching ABIs? Then put them through the ultimate compatibility crucible and pair them up on a shift at The ABI Café! Find out if your one true pairing fastcalls for each other or are just another slowburn disaster. (Maid outfits optional but recommended.)

# Trophy Case

* [x64 linux clang and gcc disagree on __int128 pass-on-stack ABI](https://github.com/rust-lang/rust/issues/54341#issuecomment-1064729606)
  * We already knew clang and rustc disagreed [because clang does a manual alignment adjustment](https://reviews.llvm.org/D28990), but we didn't seem to fully understand that the clang adjustment is actually buggy and doesn't apply to the implicit push-to-stack when passing __int128 by-val. gcc always aligns the value, even when pushing to stack, so the two desync in this case.
  * This tool was written to investigate the clang-rustc issue, and helped establish that everyone agreed on the ABI on ARM64, where __int128 is essentially part of the *hardware's* ABI due to it showing up in the layout for saving/restoring SIMD register state. As a result, [rust's libc crate now exposes typedefs for __int128 on those platforms](https://github.com/rust-lang/libc/pull/2719)
* [rustc_codegen_cranelift ICE on passing 11 bools by-val](https://github.com/bjorn3/rustc_codegen_cranelift/issues/1234)
