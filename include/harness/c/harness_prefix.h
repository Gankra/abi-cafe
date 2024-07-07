
#define WriteBuffer void*

extern WriteBuffer CALLER_VALS;
extern WriteBuffer CALLEE_VALS;
extern void (*WRITE_VAL)(WriteBuffer, uint32_t, char*, uint32_t);
extern void (*SET_FUNC)(WriteBuffer, uint32_t);

#define set_func(vals, func_idx) SET_FUNC(vals, func_idx);
#define write_val(vals, val_idx, val) WRITE_VAL(vals, val_idx, (char*)&val, (uint32_t)sizeof(val))
