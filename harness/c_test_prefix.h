#include <inttypes.h>
#include <string.h>
#include <stdio.h>
#include <stdbool.h>

#define WriteBuffer void*

extern WriteBuffer CALLER_INPUTS;
extern WriteBuffer CALLER_OUTPUTS;
extern WriteBuffer CALLEE_INPUTS;
extern WriteBuffer CALLEE_OUTPUTS;
extern void (*WRITE)(WriteBuffer, char*, uint32_t);
extern void (*FINISHED_VAL)(WriteBuffer);
extern void (*FINISHED_FUNC)(WriteBuffer, WriteBuffer);

/*
#define cdecl __attribute__((cdecl))
#define system __attribute__((system))
#define stdcall __attribute__((stdcall))
#define fastcall __attribute__((fastcall))
#define vectorcall __attribute__((vectorcall))
#define sysv64 __attribute__((sysv64))
#define win64 __attribute__((win64))
*/

