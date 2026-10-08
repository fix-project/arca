(module
  (memory (export "memory") 1)
  (tag $payload (param i32))
  (func (export "_start")
    block $caught (result i32)
      try_table (catch $payload $caught)
        block $exception (result exnref)
          try_table (catch_all_ref $exception)
            i32.const 42
            throw $payload
          end
          unreachable
        end
        throw_ref
      end
      unreachable
    end
    i32.const 42
    i32.ne
    if
      unreachable
    end))
