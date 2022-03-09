#include "../../harness/c_test_prefix.h"

typedef struct MyStruct {
    uint64_t field0;
    uint32_t* field1;
} MyStruct;

bool i_am_opaque_to_the_test_harness(uint64_t arg0, MyStruct* arg1, MyStruct arg2);

void do_test(void) { 
    uint64_t arg0 = 0x1234567898765432;
    WRITE(CALLER_INPUTS, (char*)&arg0, (uint32_t)sizeof(arg0));
    FINISHED_VAL(CALLER_INPUTS);

    uint32_t temp1 = 0xa8f0ed12;
    MyStruct arg1 = { 0xaf3e3628b800cd32, &temp1 };
    WRITE(CALLER_INPUTS, (char*)&arg1.field0, (uint32_t)sizeof(arg1.field0));
    WRITE(CALLER_INPUTS, (char*)&arg1.field1, (uint32_t)sizeof(arg1.field1));
    FINISHED_VAL(CALLER_INPUTS);

    MyStruct arg2 = { 0xbe102623e810ad39, 0 };
    WRITE(CALLER_INPUTS, (char*)&arg2.field0, (uint32_t)sizeof(arg2.field0));
    WRITE(CALLER_INPUTS, (char*)&arg2.field1, (uint32_t)sizeof(arg2.field1));
    FINISHED_VAL(CALLER_INPUTS);

    bool output = i_am_opaque_to_the_test_harness(arg0, &arg1, arg2);
    
    WRITE(CALLER_OUTPUTS, (char*)&output, (uint32_t)sizeof(output));
    FINISHED_VAL(CALLER_OUTPUTS);

    FINISHED_FUNC(CALLER_INPUTS, CALLER_OUTPUTS);
}