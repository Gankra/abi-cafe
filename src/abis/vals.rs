use kdl_script::types::*;
use rand::Rng;
use rand_core::{RngCore, SeedableRng};
use serde::Serialize;

pub type ValIter<'a> = std::slice::Iter<'a, ValueGenerator>;
type RngImpl = rand_pcg::Pcg64;

#[derive(Debug, Clone)]
pub struct ValueTree {
    pub generator_kind: ValueGeneratorKind,
    pub funcs: Vec<FuncValues>,
}

#[derive(Debug, Clone)]
pub struct FuncValues {
    pub args: Vec<ArgValues>,
}

#[derive(Debug, Clone)]
pub struct ArgValues {
    pub vals: Vec<ValueGenerator>,
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

#[derive(Debug, Clone, Serialize)]
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
                let args = func
                    .inputs
                    .iter()
                    .chain(&func.outputs)
                    .map(|arg| {
                        let mut vals = vec![];
                        generators.build_values(types, arg.ty, &mut vals);
                        ArgValues { vals }
                    })
                    .collect();
                FuncValues { args }
            })
            .collect();

        ValueTree {
            generator_kind,
            funcs,
        }
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
    fn next(&mut self) -> ValueGenerator {
        match self {
            ValueGeneratorBuilder::Graffiti { idx } => {
                let res = ValueGenerator::Graffiti { idx: *idx };
                *idx += 1;
                res
            }
            ValueGeneratorBuilder::Random { rng } => ValueGenerator::Random {
                seed: rng.next_u64(),
            },
        }
    }

    fn build_values(
        &mut self,
        types: &TypedProgram,
        ty_idx: TyIdx,
        vals: &mut Vec<ValueGenerator>,
    ) {
        let ty = types.realize_ty(ty_idx);
        match ty {
            // Primitives and enums just have the one value
            Ty::Primitive(_) => vals.push(self.next()),
            Ty::Enum(_) => vals.push(self.next()),

            // Empty has no values
            Ty::Empty => {}

            // Alias and ref are just wrappers
            Ty::Alias(ty) => self.build_values(types, ty.real, vals),
            Ty::Ref(ty) => self.build_values(types, ty.pointee_ty, vals),

            // Struct and array are just all of their fields combined
            Ty::Struct(ty) => {
                for field in &ty.fields {
                    self.build_values(types, field.ty, vals);
                }
            }
            Ty::Array(ty) => {
                for _ in 0..ty.len {
                    self.build_values(types, ty.elem_ty, vals);
                }
            }

            // Union and Tagged need an implicit "tag" field for selecting the active variant
            Ty::Union(ty) => {
                // generate the tag value
                let tag_generator = self.next();
                let active_variant_idx = tag_generator.generate_idx(ty.fields.len());
                vals.push(tag_generator);

                // now visit the active variant
                if let Some(field) = ty.fields.get(active_variant_idx) {
                    self.build_values(types, field.ty, vals);
                }
            }
            Ty::Tagged(ty) => {
                // generate the tag value
                let tag_generator = self.next();
                let active_variant_idx = tag_generator.generate_idx(ty.variants.len());
                vals.push(tag_generator);

                // now visit the active variant
                if let Some(variant) = ty.variants.get(active_variant_idx) {
                    if let Some(fields) = &variant.fields {
                        // And all of its fields
                        for field in fields {
                            self.build_values(types, field.ty, vals);
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
                    self.build_values(types, block.real, &mut new_vals);

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
}
