
#define WriteBuffer void*

extern WriteBuffer CALLER_INPUTS;
extern WriteBuffer CALLER_OUTPUTS;
extern WriteBuffer CALLEE_INPUTS;
extern WriteBuffer CALLEE_OUTPUTS;
extern void (*WRITE_FIELD)(WriteBuffer, char*, uint32_t);
extern void (*FINISHED_VAL)(WriteBuffer);
extern void (*FINISHED_FUNC)(WriteBuffer, WriteBuffer);

#define finished_val(buffer) FINISHED_VAL(buffer)
#define finished_func(inputs, outputs) FINISHED_FUNC(inputs, outputs)
#define write_field(buffer, field) WRITE_FIELD(buffer, (char*)&field, (uint32_t)sizeof(field))
