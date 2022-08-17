#include "../../harness/c_test_prefix.h"

/*
According to the popularly shared x64 SysV ABI document
https://www.uclibc.org/docs/psABI-x86_64.pdf
3.2.3 Parameter Passing (page 18)

---------------------------------------------------------

Arguments of type __int128 offer the same operations as INTEGERs,
yet they do not fit into one general purpose register but require two registers.
For classification purposes __int128 is treated as if it were implemented
as:

typedef struct {
    long low, high;
} __int128;

with the exception that arguments of type __int128 that are stored in
memory must be aligned on a 16-byte boundary.

--------------------------------------------------------

So at least in theory, this type should be ABI-compatible with 
__int128 on x64 sysv platforms (like x64 linux). Let's try:
*/

typedef struct {
    long low, high;
} my_int128 __attribute__ ((aligned (16)));

typedef struct {
    long low, high;
} my_unaligned_int128;

typedef void (*functy1)(__int128, __int128, float, __int128, uint8_t, __int128);
typedef void (*functy2)(my_int128, my_int128, float, my_int128, uint8_t, my_int128);

void callee_native_layout(__int128* arg0, __int128* arg1, __int128* arg2) {
    WRITE_FIELD(CALLEE_INPUTS, (char*)arg0, (uint32_t)sizeof(*arg0));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)arg1, (uint32_t)sizeof(*arg1));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)arg2, (uint32_t)sizeof(*arg2));
    FINISHED_VAL(CALLEE_INPUTS);

    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);
}
void callee_emulated_layout(my_int128* arg0, my_int128* arg1, my_int128* arg2) {
    WRITE_FIELD(CALLEE_INPUTS, (char*)arg0, (uint32_t)sizeof(*arg0));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)arg1, (uint32_t)sizeof(*arg1));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)arg2, (uint32_t)sizeof(*arg2));
    FINISHED_VAL(CALLEE_INPUTS);

    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);
}
void callee_unaligned_emulated_layout(my_unaligned_int128* arg0, my_unaligned_int128* arg1, my_unaligned_int128* arg2) {
    WRITE_FIELD(CALLEE_INPUTS, (char*)arg0, (uint32_t)sizeof(*arg0));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)arg1, (uint32_t)sizeof(*arg1));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)arg2, (uint32_t)sizeof(*arg2));
    FINISHED_VAL(CALLEE_INPUTS);

    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);
}

// Native Version
void native_to_native(__int128 arg0, __int128 arg1, float arg2, __int128 arg3, uint8_t arg4, __int128 arg5) {
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg0, (uint32_t)sizeof(arg0));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg1, (uint32_t)sizeof(arg1));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg2, (uint32_t)sizeof(arg2));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg3, (uint32_t)sizeof(arg3));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg4, (uint32_t)sizeof(arg4));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg5, (uint32_t)sizeof(arg5));
    FINISHED_VAL(CALLEE_INPUTS);

    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);
}

// Emulated Version
void native_to_emulated(my_int128 arg0, my_int128 arg1, float arg2, my_int128 arg3, uint8_t arg4, my_int128 arg5) {
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg0, (uint32_t)sizeof(arg0));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg1, (uint32_t)sizeof(arg1));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg2, (uint32_t)sizeof(arg2));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg3, (uint32_t)sizeof(arg3));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg4, (uint32_t)sizeof(arg4));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg5, (uint32_t)sizeof(arg5));
    FINISHED_VAL(CALLEE_INPUTS);

    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);
}

// Unaligned Emulated Version
void native_to_unaligned_emulated(my_unaligned_int128 arg0, my_unaligned_int128 arg1, float arg2, my_unaligned_int128 arg3, uint8_t arg4, my_unaligned_int128 arg5) {
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg0, (uint32_t)sizeof(arg0));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg1, (uint32_t)sizeof(arg1));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg2, (uint32_t)sizeof(arg2));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg3, (uint32_t)sizeof(arg3));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg4, (uint32_t)sizeof(arg4));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg5, (uint32_t)sizeof(arg5));
    FINISHED_VAL(CALLEE_INPUTS);

    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);
}


// Native Version
void emulated_to_native(__int128 arg0, __int128 arg1, float arg2, __int128 arg3, uint8_t arg4, __int128 arg5) {
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg0, (uint32_t)sizeof(arg0));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg1, (uint32_t)sizeof(arg1));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg2, (uint32_t)sizeof(arg2));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg3, (uint32_t)sizeof(arg3));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg4, (uint32_t)sizeof(arg4));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg5, (uint32_t)sizeof(arg5));
    FINISHED_VAL(CALLEE_INPUTS);

    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);
}

// Emulated Version
void emulated_to_emulated(my_int128 arg0, my_int128 arg1, float arg2, my_int128 arg3, uint8_t arg4, my_int128 arg5) {
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg0, (uint32_t)sizeof(arg0));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg1, (uint32_t)sizeof(arg1));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg2, (uint32_t)sizeof(arg2));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg3, (uint32_t)sizeof(arg3));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg4, (uint32_t)sizeof(arg4));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg5, (uint32_t)sizeof(arg5));
    FINISHED_VAL(CALLEE_INPUTS);

    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);
}

// Unaligned Emulated Version
void emulated_to_unaligned_emulated(my_unaligned_int128 arg0, my_unaligned_int128 arg1, float arg2, my_unaligned_int128 arg3, uint8_t arg4, my_unaligned_int128 arg5) {
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg0, (uint32_t)sizeof(arg0));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg1, (uint32_t)sizeof(arg1));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg2, (uint32_t)sizeof(arg2));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg3, (uint32_t)sizeof(arg3));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg4, (uint32_t)sizeof(arg4));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg5, (uint32_t)sizeof(arg5));
    FINISHED_VAL(CALLEE_INPUTS);

    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);
}

// Native Version
void unaligned_emulated_to_native(__int128 arg0, __int128 arg1, float arg2, __int128 arg3, uint8_t arg4, __int128 arg5) {
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg0, (uint32_t)sizeof(arg0));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg1, (uint32_t)sizeof(arg1));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg2, (uint32_t)sizeof(arg2));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg3, (uint32_t)sizeof(arg3));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg4, (uint32_t)sizeof(arg4));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg5, (uint32_t)sizeof(arg5));
    FINISHED_VAL(CALLEE_INPUTS);

    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);
}

// Emulated Version
void unaligned_emulated_to_emulated(my_int128 arg0, my_int128 arg1, float arg2, my_int128 arg3, uint8_t arg4, my_int128 arg5) {
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg0, (uint32_t)sizeof(arg0));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg1, (uint32_t)sizeof(arg1));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg2, (uint32_t)sizeof(arg2));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg3, (uint32_t)sizeof(arg3));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg4, (uint32_t)sizeof(arg4));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg5, (uint32_t)sizeof(arg5));
    FINISHED_VAL(CALLEE_INPUTS);

    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);
}

// Unaligned Emulated Version
void unaligned_emulated_to_unaligned_emulated(my_unaligned_int128 arg0, my_unaligned_int128 arg1, float arg2, my_unaligned_int128 arg3, uint8_t arg4, my_unaligned_int128 arg5) {
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg0, (uint32_t)sizeof(arg0));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg1, (uint32_t)sizeof(arg1));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg2, (uint32_t)sizeof(arg2));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg3, (uint32_t)sizeof(arg3));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg4, (uint32_t)sizeof(arg4));
    FINISHED_VAL(CALLEE_INPUTS);
    WRITE_FIELD(CALLEE_INPUTS, (char*)&arg5, (uint32_t)sizeof(arg5));
    FINISHED_VAL(CALLEE_INPUTS);

    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);
}