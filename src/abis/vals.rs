use kdl_script::types::*;
use rand::Rng;
use rand_core::{RngCore, SeedableRng};
use serde::Serialize;

use crate::TestOptions;

type RngImpl = rand_pcg::Pcg64;

#[derive(Debug, Clone)]
pub struct ValueTree {
    pub generator_kind: ValueGeneratorKind,
    pub funcs: Vec<FuncValues>,
}

#[derive(Debug, Clone)]
pub struct FuncValues {
    pub func_name: String,
    pub args: Vec<ArgValues>,
}

#[derive(Debug, Clone)]
pub struct ArgValues {
    pub arg_name: String,
    pub ty: TyIdx,
    pub is_input: bool,
    pub vals: Vec<Value>,
}

#[derive(Debug, Clone)]
pub struct FuncValuesIter<'a> {
    tree: &'a ValueTree,
    func_idx: usize,
    arg_idx: usize,
}

#[derive(Debug, Clone)]
pub struct ArgValuesIter<'a> {
    tree: &'a ValueTree,
    func_idx: usize,
    pub arg_idx: usize,
    val_idx: usize,
}

#[derive(Debug, Clone)]
pub struct ValueRef<'a> {
    tree: &'a ValueTree,
    func_idx: usize,
    arg_idx: usize,
    val_idx: usize,
}

#[derive(Debug, Clone)]
pub struct Value {
    pub val: ValueGenerator,
    pub ty: TyIdx,
    pub path: String,
}

#[derive(Debug, Clone)]
pub enum ValueGenerator {
    Graffiti { idx: u64 },
    Random { seed: u64 },
}

#[derive(Debug, Clone)]
enum ValueGeneratorBuilder {
    Graffiti { idx: u64 },
    Random { rng: RngImpl },
}

#[derive(Debug, Copy, Clone, Serialize)]
pub enum ValueGeneratorKind {
    Graffiti,
    Random { seed: u64 },
}

impl ValueTree {
    /// Create the ValueTree for an entire program
    pub fn new(types: &TypedProgram, generator_kind: ValueGeneratorKind) -> Self {
        let mut generators = generator_kind.builder();
        // Construct value generators for every function
        let funcs = types
            .all_funcs()
            .map(|func_idx| {
                let func = types.realize_func(func_idx);
                let func_name = func.name.to_string();
                let args = func
                    .inputs
                    .iter()
                    .map(|arg| (true, arg))
                    .chain(func.outputs.iter().map(|arg| (false, arg)))
                    .map(|(is_input, arg)| {
                        let mut vals = vec![];
                        let arg_name = arg.name.to_string();
                        generators.build_values(types, arg.ty, &mut vals, arg_name.clone());
                        ArgValues {
                            ty: arg.ty,
                            arg_name,
                            is_input,
                            vals,
                        }
                    })
                    .collect();
                FuncValues { func_name, args }
            })
            .collect();

        ValueTree {
            generator_kind,
            funcs,
        }
    }

    #[track_caller]
    pub fn at_func(&self, func_idx: usize) -> FuncValuesIter {
        assert!(
            func_idx < self.funcs.len(),
            "internal error: ValueTree func_idx exceeded"
        );
        FuncValuesIter {
            tree: self,
            func_idx,
            arg_idx: 0,
        }
    }
}

impl<'a> FuncValuesIter<'a> {
    #[track_caller]
    pub fn next_arg(&mut self) -> ArgValuesIter<'a> {
        let Self {
            tree,
            func_idx,
            arg_idx,
        } = *self;
        assert!(
            arg_idx < tree.funcs[func_idx].args.len(),
            "internal error: ValueTree arg_idx exceeded"
        );
        self.arg_idx += 1;
        ArgValuesIter {
            tree,
            func_idx,
            arg_idx,
            val_idx: 0,
        }
    }
}

impl<'a> ArgValuesIter<'a> {
    #[track_caller]
    pub fn next_val(&mut self) -> ValueRef<'a> {
        let Self {
            tree,
            func_idx,
            arg_idx,
            val_idx,
        } = *self;
        assert!(
            val_idx < tree.funcs[func_idx].args[arg_idx].vals.len(),
            "internal error: ValueTree val_idx exceeded"
        );
        self.val_idx += 1;
        ValueRef {
            tree,
            func_idx,
            arg_idx,
            val_idx,
        }
    }

    pub fn should_write_arg(&self, options: &TestOptions) -> bool {
        options
            .functions
            .should_write_arg(self.func_idx, self.arg_idx)
    }

    pub fn arg(&self) -> &'a ArgValues {
        &self.tree.funcs[self.func_idx].args[self.arg_idx]
    }
}

impl<'a> ValueRef<'a> {
    pub fn should_write_val(&self, options: &TestOptions) -> bool {
        options
            .functions
            .should_write_val(self.func_idx, self.arg_idx, self.val_idx)
    }
}
impl<'a> std::ops::Deref for ValueRef<'a> {
    type Target = Value;
    fn deref(&self) -> &Self::Target {
        &self.tree.funcs[self.func_idx].args[self.arg_idx].vals[self.val_idx]
    }
}
impl std::ops::Deref for Value {
    type Target = ValueGenerator;
    fn deref(&self) -> &Self::Target {
        &self.val
    }
}

impl ValueGeneratorKind {
    fn builder(&self) -> ValueGeneratorBuilder {
        match self {
            ValueGeneratorKind::Graffiti => ValueGeneratorBuilder::Graffiti { idx: 0 },
            // We use the given seed to construct an RNG, and make new RNG seeds with it.
            // This isn't to "increase randomness" or anything, but instead to create N
            // random streams of bytes that can be repeatably and independently queried,
            // while still having them all deterministically derived from the root seed.
            ValueGeneratorKind::Random { seed } => ValueGeneratorBuilder::Random {
                rng: RngImpl::seed_from_u64(*seed),
            },
        }
    }
}

impl ValueGeneratorBuilder {
    fn next(&mut self, ty: TyIdx, path: String) -> Value {
        let val = match self {
            ValueGeneratorBuilder::Graffiti { idx } => {
                let res = ValueGenerator::Graffiti { idx: *idx };
                *idx += 1;
                res
            }
            ValueGeneratorBuilder::Random { rng } => ValueGenerator::Random {
                seed: rng.next_u64(),
            },
        };
        Value { val, ty, path }
    }

    fn build_values(
        &mut self,
        types: &TypedProgram,
        ty_idx: TyIdx,
        vals: &mut Vec<Value>,
        path: String,
    ) {
        let ty = types.realize_ty(ty_idx);
        match ty {
            // Primitives and enums just have the one value
            Ty::Primitive(_) => vals.push(self.next(ty_idx, path)),
            Ty::Enum(_) => vals.push(self.next(ty_idx, path)),

            // Empty has no values
            Ty::Empty => {}

            // Alias and ref are just wrappers
            Ty::Alias(ty) => self.build_values(types, ty.real, vals, path),
            Ty::Ref(ty) => {
                let new_path = format!("{path}.*");
                self.build_values(types, ty.pointee_ty, vals, new_path)
            }

            // Struct and array are just all of their fields combined
            Ty::Struct(ty) => {
                for field in &ty.fields {
                    let field_name = &field.ident;
                    let new_path = format!("{path}.{field_name}");
                    self.build_values(types, field.ty, vals, new_path);
                }
            }
            Ty::Array(ty) => {
                for idx in 0..ty.len {
                    let new_path = format!("{path}[{idx}]");
                    self.build_values(types, ty.elem_ty, vals, new_path);
                }
            }

            // Union and Tagged need an implicit "tag" field for selecting the active variant
            Ty::Union(ty) => {
                // generate the tag value
                let tag_generator = self.next(ty_idx, path.clone());
                let active_variant_idx = tag_generator.generate_idx(ty.fields.len());
                vals.push(tag_generator);

                // now visit the active variant
                if let Some(field) = ty.fields.get(active_variant_idx) {
                    let field_name = &field.ident;
                    let new_path = format!("{path}.{field_name}");
                    self.build_values(types, field.ty, vals, new_path);
                }
            }
            Ty::Tagged(ty) => {
                // generate the tag value
                let tag_generator = self.next(ty_idx, path.clone());
                let active_variant_idx = tag_generator.generate_idx(ty.variants.len());
                vals.push(tag_generator);

                // now visit the active variant
                if let Some(variant) = ty.variants.get(active_variant_idx) {
                    if let Some(fields) = &variant.fields {
                        // And all of its fields
                        for field in fields {
                            let variant_name = &variant.name;
                            let field_name = &field.ident;
                            let new_path = format!("{path}.{variant_name}.{field_name}");
                            self.build_values(types, field.ty, vals, new_path);
                        }
                    }
                }
            }

            // Pun ty is similar to a union, but for integrity we want to enforce that all paths
            // produce the same number of values
            Ty::Pun(ty) => {
                let mut out_vals = None::<Vec<_>>;
                let saved_self = self.clone();
                for block in &ty.blocks {
                    // Every time we re-enter here, restore our state to before we started.
                    // This ensures our state is mutated for good after the last iteration,
                    // but the same state is used for each one.
                    *self = saved_self.clone();

                    // Shove values into a temp buffer instead of the main one
                    let mut new_vals = vec![];
                    self.build_values(types, block.real, &mut new_vals, path.clone());

                    // If there are multiple blocks, check that this new one matches
                    // all the other ones in length (making the pun semantically comprehensible)
                    if let Some(old_vals) = out_vals {
                        assert!(old_vals.len() != new_vals.len(), "pun {} had cases with different numbers of values (~leaf fields), this is unsupported", ty.name);
                    }

                    // Finally store the result
                    out_vals = Some(new_vals);
                }

                // If we visited any blocks, properly add the values to the output
                if let Some(out_vals) = out_vals {
                    vals.extend(out_vals);
                }
            }
        }
    }
}

impl ValueGenerator {
    pub fn fill_bytes(&self, output: &mut [u8]) {
        match self {
            ValueGenerator::Graffiti { idx } => {
                // Graffiti bytes:
                // high nibble is the field index (wrapping)
                // low nibble is the byte index (wrapping)
                for (byte_idx, byte) in output.iter_mut().enumerate() {
                    *byte = ((*idx as u8) << 4) | ((byte_idx as u8) & 0b1111);
                }
            }
            ValueGenerator::Random { seed } => {
                // Construct an RNG from this seed and ask it to fill in the bytes
                let mut rng = RngImpl::seed_from_u64(*seed);
                rng.fill_bytes(output);
            }
        }
    }

    pub fn select_val<'a, T>(&self, options: &'a [T]) -> Option<&'a T> {
        let idx = self.generate_idx(options.len());
        options.get(idx)
    }

    // Generate an index in the range 0..len
    pub fn generate_idx(&self, len: usize) -> usize {
        // Convenient special case for empty lists
        if len == 0 {
            return 0;
        }
        let mut rng = match self {
            ValueGenerator::Graffiti { .. } => {
                // To turn our pattern value into a fairly evenly distributed selection
                // of the possible values in the range, just generate a grafitti u64 and
                // use it as the seed for an rng, then ask rand to figure it out!
                let seed = self.generate_u64();
                RngImpl::seed_from_u64(seed)
            }
            ValueGenerator::Random { seed } => RngImpl::seed_from_u64(*seed),
        };
        rng.gen_range(0..len)
    }
    pub fn generate_u8(&self) -> u8 {
        let mut buf = [0; 1];
        self.fill_bytes(&mut buf);
        u8::from_le_bytes(buf)
    }
    pub fn generate_u16(&self) -> u16 {
        let mut buf = [0; 2];
        self.fill_bytes(&mut buf);
        u16::from_le_bytes(buf)
    }
    pub fn generate_u32(&self) -> u32 {
        let mut buf = [0; 4];
        self.fill_bytes(&mut buf);
        u32::from_le_bytes(buf)
    }
    pub fn generate_u64(&self) -> u64 {
        let mut buf = [0; 8];
        self.fill_bytes(&mut buf);
        u64::from_le_bytes(buf)
    }
    pub fn generate_u128(&self) -> u128 {
        let mut buf = [0; 16];
        self.fill_bytes(&mut buf);
        u128::from_le_bytes(buf)
    }
    pub fn generate_i8(&self) -> i8 {
        self.generate_u8() as i8
    }
    pub fn generate_i16(&self) -> i16 {
        self.generate_u16() as i16
    }
    pub fn generate_i32(&self) -> i32 {
        self.generate_u32() as i32
    }
    pub fn generate_i64(&self) -> i64 {
        self.generate_u64() as i64
    }
    pub fn generate_i128(&self) -> i128 {
        self.generate_u128() as i128
    }
}
