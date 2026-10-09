#include "combine-modules.h"

#include <cstdio>
#include <memory>
#include <string>
#include <unordered_set>
#include <vector>

#include "wabt/cast.h"
#include "wabt/ir.h"

namespace wabt {

Result CombineModules(const std::vector<Module *> &modules, Module *output,
                      bool rename_export_conflicts) {
  std::unordered_set<std::string> names;
  for (auto *module : modules)
    names.insert(module->name);
  ModuleFieldList definitions;
  std::vector<Var> starts;
  std::unordered_set<std::string> exports;
  for (auto *module : modules) {
    while (!module->fields.empty()) {
      auto field = module->fields.extract_front();
      if (field->type() == ModuleFieldType::Import) {
        if (!names.count(
                cast<ImportModuleField>(field.get())->import->module_name)) {
          output->AppendField(std::move(field));
        }
      } else if (field->type() == ModuleFieldType::Start) {
        starts.push_back(cast<StartModuleField>(field.get())->start);
      } else {
        if (field->type() == ModuleFieldType::Export) {
          auto &name = cast<ExportModuleField>(field.get())->export_.name;
          if (!exports.insert(name).second) {
            if (!rename_export_conflicts) {
              fprintf(stderr, "Duplicate export: %s\n", name.c_str());
              return Result::Error;
            }
            name = module->name + "." + name;
            const std::string prefix = name;
            for (size_t suffix = 1; !exports.insert(name).second; ++suffix) {
              name = prefix + "." + std::to_string(suffix);
            }
          }
        }
        definitions.push_back(std::move(field));
      }
    }
  }
  output->AppendFields(&definitions);
  if (starts.size() == 1) {
    output->AppendField(std::make_unique<StartModuleField>(starts.front()));
  } else if (!starts.empty()) {
    auto initializer = std::make_unique<FuncModuleField>();
    for (const auto &start : starts) {
      initializer->func.exprs.push_back(std::make_unique<CallExpr>(start));
    }
    const Index index = output->funcs.size();
    output->AppendField(std::move(initializer));
    output->AppendField(
        std::make_unique<StartModuleField>(Var(index, Location())));
  }
  return Result::Ok;
}

} // namespace wabt
