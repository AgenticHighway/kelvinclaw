(module
  (import "memory_host" "kv_get" (func $kv_get (param i32) (result i32)))
  (import "memory_host" "kv_put" (func $kv_put (param i32) (result i32)))
  (import "memory_host" "blob_get" (func $blob_get (param i32) (result i32)))
  (import "memory_host" "blob_put" (func $blob_put (param i32) (result i32)))
  (import "memory_host" "emit_metric" (func $emit_metric (param i32) (result i32)))
  (import "memory_host" "log" (func $log (param i32) (result i32)))
  (import "memory_host" "clock_now_ms" (func $clock_now_ms (result i64)))

  (func (export "handle_upsert") (result i32)
    i32.const 0
    call $kv_put
    drop
    i32.const 0
  )

  (func (export "handle_query") (result i32)
    i32.const 0
    call $kv_get
    drop
    i32.const 0
  )

  (func (export "handle_read") (result i32)
    call $clock_now_ms
    drop
    i32.const 0
  )

  (func (export "handle_delete") (result i32)
    i32.const 0
  )

  (func (export "handle_health") (result i32)
    i32.const 0
  )
)
