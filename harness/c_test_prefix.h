#include <inttypes.h>
#include <stdio.h>

#define WriteBuffer void*

extern WriteBuffer CALLER_INPUTS;
extern WriteBuffer CALLER_OUTPUTS;
extern WriteBuffer CALLEE_INPUTS;
extern WriteBuffer CALLEE_OUTPUTS;
extern void (*WRITE)(WriteBuffer, char*, uint32_t);
