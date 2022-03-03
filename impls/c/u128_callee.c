#include <inttypes.h>
#include <stdio.h>

#define WriteBuffer void*

    extern WriteBuffer CALLER_INPUTS;
    extern WriteBuffer CALLER_OUTPUTS;
    extern WriteBuffer CALLEE_INPUTS;
    extern WriteBuffer CALLEE_OUTPUTS;
    extern (*WRITE)(WriteBuffer, char*, uint32_t) ;

uint64_t u128_by_val(uint64_t input) {
    printf("callee inputs:\n");
    printf("%" PRIu64 "\n", input);
    WRITE(CALLEE_INPUTS, &input, sizeof(input));
    printf("\n");

    int64_t output = 1534587892765432;
    
    printf("callee outputs:\n");
    printf("%" PRIu64 "\n", output);
    WRITE(CALLEE_OUTPUTS, &output, sizeof(output));
    printf("\n");

    return output;
}