#include "combine-modules.h"

#include <map>
#include <memory>
#include <string>
#include <iostream> 

#include "wabt/cast.h"
#include "wabt/expr-visitor.h"
#include "wabt/ir.h"

namespace wabt {

Result CombineModules(Module* module, Module* libmodule, Module* result) {
  ModuleFieldList temp_module;
  ModuleFieldList temp_libmodule;

  // Append imports of module excluding imports from libmodule 
  while (!module->fields.empty()) {
    const ModuleField* field = &module->fields.front();
    if (field->type() == ModuleFieldType::Import) {
      auto field_uniq_ptr = cast<ImportModuleField>(std::move(module->fields.extract_front())); 
      if (field_uniq_ptr->import->module_name != libmodule->name) {
        result->AppendField(std::move(field_uniq_ptr));
      }
    } else {
      temp_module.push_back(std::move(module->fields.extract_front()));
    }
  }

  // Append imports of libmodule excluding imports from module 
  while (!libmodule->fields.empty()) {
    const ModuleField* field = &libmodule->fields.front();
    if (field->type() == ModuleFieldType::Import) {
      auto field_uniq_ptr = cast<ImportModuleField>(std::move(libmodule->fields.extract_front()));
      if (field_uniq_ptr->import->module_name != module->name) {
        result->AppendField(std::move(field_uniq_ptr));
      }
    } else {
      temp_libmodule.push_back(std::move(libmodule->fields.extract_front()));
    }
  }

  // Append the rest of module fields
  result->AppendFields(&temp_module);

  // Append the rest of libmodule fields
  result->AppendFields(&temp_libmodule);
  
  return Result::Ok;
}
}
