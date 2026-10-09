#ifndef WABT_COMBINE_MODULES_H_
#define WABT_COMBINE_MODULES_H_

#include "wabt/common.h"
#include <vector>

namespace wabt {
struct Module;
Result CombineModules(const std::vector<Module *> &modules, Module *output,
                      bool rename_export_conflicts = false);
} // namespace wabt

#endif
