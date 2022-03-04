#include "../../harness/c_test_prefix.h"

typedef struct MyStruct {
    uint64_t field0;
    uint32_t* field1;
} MyStruct;

bool i_am_opaque_to_the_test_harness(uint64_t arg0, MyStruct* arg1, MyStruct arg2) {
    WRITE(CALLEE_INPUTS, (char*)&arg0, (uint32_t)sizeof(arg0));
    FINISHED_VAL(CALLEE_INPUTS);

    WRITE(CALLEE_INPUTS, (char*)&arg1->field0, (uint32_t)sizeof(arg1->field0));
    WRITE(CALLEE_INPUTS, (char*)&arg1->field1, (uint32_t)sizeof(arg1->field1));
    FINISHED_VAL(CALLEE_INPUTS);

    WRITE(CALLEE_INPUTS, (char*)&arg2.field0, (uint32_t)sizeof(arg2.field0));
    WRITE(CALLEE_INPUTS, (char*)&arg2.field1, (uint32_t)sizeof(arg2.field1));
    FINISHED_VAL(CALLEE_INPUTS);

    bool output = true;
    
    WRITE(CALLEE_OUTPUTS, (char*)&output, (uint32_t)sizeof(output));
    FINISHED_VAL(CALLEE_OUTPUTS);

    FINISHED_FUNC(CALLEE_INPUTS, CALLEE_OUTPUTS);
    return output;
}