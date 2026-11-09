#pragma once
#include <cassert>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <new>
#include <type_traits>
#include <utility>
namespace fixcpp {
class Ref;
struct alignas(32) Cell {
  unsigned char bytes[32];
  operator Ref() const noexcept;
  Cell &operator=(const Ref &) noexcept;
  Cell &operator=(const Cell &) noexcept;
};
extern "C" void cell_copy(Cell *, const Cell *) noexcept;
extern "C" void cell_clear(Cell *) noexcept;
extern "C" unsigned cell_equal(const Cell *, const Cell *) noexcept;
Cell *allocate() noexcept;
void release(Cell *) noexcept;
class Ref {
  Cell *slot_;

public:
  Ref() noexcept : slot_(allocate()) {}
  Ref(std::nullptr_t) noexcept : Ref() {}
  Ref(const Ref &other) noexcept : Ref() {
    if (other.slot_)
      cell_copy(slot_, other.slot_);
  }
  Ref(Ref &&other) noexcept : slot_(std::exchange(other.slot_, nullptr)) {}
  ~Ref() { release(slot_); }
  Cell *slot() const noexcept { return slot_; }
  static Ref from_cell(const Cell *cell) noexcept {
    Ref r;
    cell_copy(r.slot_, cell);
    return r;
  }
  Ref &operator=(const Ref &other) noexcept {
    if (!slot_)
      slot_ = allocate();
    if (other.slot_)
      cell_copy(slot_, other.slot_);
    else
      cell_clear(slot_);
    return *this;
  }
  bool operator==(const Ref &other) const noexcept {
    return cell_equal(slot_, other.slot_);
  }
  explicit operator bool() const noexcept {
    static const Cell zero{};
    return slot_ && !cell_equal(slot_, &zero);
  }
};
inline Cell::operator Ref() const noexcept { return Ref::from_cell(this); }
inline Cell &Cell::operator=(const Ref &r) noexcept {
  cell_copy(this, r.slot());
  return *this;
}
inline Cell &Cell::operator=(const Cell &r) noexcept {
  cell_copy(this, &r);
  return *this;
}
template <class T>
using Raw = std::conditional_t<std::is_same_v<T, Ref>, Cell *, T>;
template <class T> Raw<T> raw(const T &value) noexcept {
  if constexpr (std::is_same_v<T, Ref>)
    return value.slot();
  else
    return value;
}
template <class Signature> struct Import;
template <class Result, class Context, class... Arguments>
struct Import<Result(Context, Arguments...)> {
  void (*symbol)();
  Result operator()(Context context, Arguments... arguments) const {
    if constexpr (std::is_same_v<Result, Ref>) {
      using Function = void (*)(Context, Cell *, Raw<Arguments>...);
      Ref result;
      reinterpret_cast<Function>(symbol)(context, result.slot(),
                                         raw(arguments)...);
      return result;
    } else {
      using Function = Result (*)(Context, Raw<Arguments>...);
      return reinterpret_cast<Function>(symbol)(context, raw(arguments)...);
    }
  }
};
struct PayloadBase {
  unsigned owners = 1;
  const void *tag;
  explicit PayloadBase(const void *t) : tag(t) {}
  virtual ~PayloadBase() = default;
  virtual const void *data() const = 0;
  virtual std::uint32_t size() const = 0;
};
template <class T> struct Payload final : PayloadBase {
  T value;
  Payload(const void *tag, const T &v) : PayloadBase(tag), value(v) {}
  const void *data() const override { return &value; }
  std::uint32_t size() const override { return sizeof(T); }
};
class PayloadOwner {
  PayloadBase *payload_ = nullptr;
  void retain() {
    if (payload_)
      ++payload_->owners;
  }
  void drop() {
    if (payload_ && !--payload_->owners)
      delete payload_;
  }

public:
  PayloadOwner() = default;
  explicit PayloadOwner(PayloadBase *p) : payload_(p) {}
  PayloadOwner(const PayloadOwner &x) noexcept : payload_(x.payload_) {
    retain();
  }
  PayloadOwner(PayloadOwner &&x) noexcept
      : payload_(std::exchange(x.payload_, nullptr)) {}
  ~PayloadOwner() { drop(); }
  PayloadOwner &operator=(const PayloadOwner &x) noexcept {
    auto *p = x.payload_;
    if (p)
      ++p->owners;
    drop();
    payload_ = p;
    return *this;
  }
  const void *get() const {
    assert(payload_);
    return payload_->data();
  }
};
struct Exception {
  const void *tag = nullptr;
  std::uint32_t size = 0;
  PayloadOwner data;
  Exception() = default;
  explicit Exception(PayloadBase *p) : tag(p->tag), size(p->size()), data(p) {}
  Exception(const void *tag, std::uint32_t size, const PayloadOwner &data)
      : tag(tag), size(size), data(data) {}
};
inline Exception current;
struct Target;
inline Target *target = nullptr;
struct Target {
  Target *outer = target;
  Exception previous = current;
  ~Target() {
    current = previous;
    if (target == this)
      target = outer;
  }
};
struct Thrown {
  Target *destination;
  Exception payload;
};
inline void catch_enter(const Thrown &e, Target &frame) {
  if (e.destination != &frame)
    throw;
  current = e.payload;
}
template <class T> void store_exception(const void *tag, const T &value) {
  current = Exception(new Payload<std::remove_cv_t<T>>(tag, value));
}
inline void empty_exception(const void *tag) { store_exception(tag, 0); }
[[noreturn]] void null_exception();
inline void restore_exception(const Exception &e) {
  if (!e.tag)
    null_exception();
  current = e;
}
struct TailBuffer {
  alignas(std::max_align_t) unsigned char bytes[1024]{};
  void (*destroy)(void *) = nullptr;
  void clear() {
    if (destroy) {
      destroy(bytes);
      destroy = nullptr;
    }
    std::memset(bytes, 0, sizeof(bytes));
  }
  ~TailBuffer() { clear(); }
  template <class T> void store(const T &value) {
    static_assert(sizeof(T) <= sizeof(bytes));
    static_assert(alignof(T) <= alignof(TailBuffer));
    clear();
    new (bytes) T(value);
    destroy = [](void *p) { static_cast<T *>(p)->~T(); };
  }
  template <class T> void load(T &value) {
    value = *reinterpret_cast<const T *>(bytes);
    clear();
  }
};
template <class T>
void table_copy(T *destination, const T *source, std::size_t count) {
  if (reinterpret_cast<std::uintptr_t>(destination) <
      reinterpret_cast<std::uintptr_t>(source)) {
    for (std::size_t i = 0; i < count; ++i)
      destination[i] = source[i];
  } else {
    while (count) {
      --count;
      destination[count] = source[count];
    }
  }
}

template <class D, class S>
void *copy(D *destination, const S *source, std::size_t count) {
  static_assert(std::is_trivially_copyable_v<D> &&
                std::is_trivially_copyable_v<S>);
  return std::memcpy(destination, source, count);
}
template <class T>
void copy(TailBuffer *destination, const T *source, std::size_t count) {
  assert(count == sizeof(T));
  destination->store(*source);
}
template <class T>
void copy(T *destination, TailBuffer *source, std::size_t count) {
  assert(count == sizeof(T));
  source->load(*destination);
}
template <class T>
void copy(T *destination, const Exception &source, std::size_t count) {
  assert(count == sizeof(T));
  *destination = *static_cast<const T *>(source.data.get());
}
inline void copy(PayloadOwner *destination, const Exception &source,
                 std::size_t) {
  *destination = source.data;
}
template <class T>
void load_exception(const void *tag, std::uint32_t, const T *value) {
  store_exception(tag, *value);
}
inline void load_exception(const void *tag, std::uint32_t, const void *) {
  empty_exception(tag);
}
inline void load_exception(const void *, std::uint32_t,
                           const Exception &value) {
  restore_exception(value);
}
inline void load_exception(const void *tag, std::uint32_t size,
                           const PayloadOwner &value) {
  current = Exception(tag, size, value);
}
} // namespace fixcpp
#define WASM_RT_TRY(frame) try
#define WASM_RT_CATCH(frame) catch (const fixcpp::Thrown &wasm_exception)
#define WASM_RT_CATCH_ENTER(frame) fixcpp::catch_enter(wasm_exception, frame)
#define WASM_RT_DELEGATE(label)                                                \
  throw fixcpp::Thrown { &label##_unwind_target, fixcpp::current }
#define WASM_RT_EXCEPTION_SAVE(name)                                           \
  fixcpp::Exception name = fixcpp::current;                                    \
  const auto name##_tag = name.tag;                                            \
  const auto name##_size = name.size
#define WASM_RT_TAIL_STORAGE(name)                                             \
  fixcpp::TailBuffer name##_storage;                                           \
  auto *name = &name##_storage
#define WASM_RT_TAIL_STACK_TYPE fixcpp::TailBuffer *
#define wasm_rt_exception() fixcpp::current
#define wasm_rt_exception_size() fixcpp::current.size
#define wasm_rt_load_exception fixcpp::load_exception
inline void *memmove(fixcpp::Cell *destination, const fixcpp::Cell *source,
                     std::size_t bytes) {
  assert(bytes % sizeof(fixcpp::Ref) == 0);
  fixcpp::table_copy(destination, source, bytes / sizeof(fixcpp::Ref));
  return destination;
}

#define WASM_RT_IMPORT(result, name, parameters)                               \
  extern void name##_raw() __asm__(#name);                                     \
  inline constexpr fixcpp::Import<result parameters> name __asm__(#name        \
                                                                  "_cpp") {    \
    name##_raw                                                                 \
  }
#define WASM_RT_LOCAL_QUALIFIER
#define wasm2c_module_instantiate cpp_wasm2c_module_instantiate
#define wasm2c_module_free cpp_wasm2c_module_free
#define w2c_module_0x5Ffix_apply cpp_w2c_module_0x5Ffix_apply
#define wasm_rt_grow_externref_table cpp_wasm_rt_grow_externref_table

#define WEAK_FUNC_DECL(func, fallback)                                         \
  __attribute__((weak)) void func(void **instance_ptr,                         \
                                  fixcpp::TailBuffer *tail_call_stack,         \
                                  wasm_rt_tailcallee_t *next)
