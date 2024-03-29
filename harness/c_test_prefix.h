#include <inttypes.h>
#include <string.h>
#include <stdio.h>
#include <stdbool.h>

#define WriteBuffer void*

extern WriteBuffer CALLER_INPUTS;
extern WriteBuffer CALLER_OUTPUTS;
extern WriteBuffer CALLEE_INPUTS;
extern WriteBuffer CALLEE_OUTPUTS;
extern void (*WRITE_FIELD)(WriteBuffer, char*, uint32_t);
extern void (*FINISHED_VAL)(WriteBuffer);
extern void (*FINISHED_FUNC)(WriteBuffer, WriteBuffer);

