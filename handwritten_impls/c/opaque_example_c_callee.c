#include "../../harness/c_test_prefix.h"

uint64_t i_am_opaque_to_the_test_harness(uint64_t input) {
    // printf("callee inputs:\n");
    // printf("%" PRIu64 "\n", input);
    WRITE(CALLEE_INPUTS, (char*)&input, sizeof(input));
    FINISHED_VAL(CALLEE_INPUTS);
    // printf("\n");

    int64_t output = 1534587892765432;
    
    // printf("callee outputs:\n");
    // printf("%" PRIu64 "\n", output);
    WRITE(CALLEE_OUTPUTS, (char*)&output, sizeof(output));
    FINISHED_VAL(CALLEE_OUTPUTS);
    // printf("\n");

    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);
    return output;
}