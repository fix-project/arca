/*
 * Copyright 2016 WebAssembly Community Group participants
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#include "resolve-imports.h"

#include <cassert>
#include <cstdio>
#include <iostream>
#include <vector>

#include "src/cast.h"
#include "src/expr-visitor.h"
#include "src/ir.h"

using namespace std;

namespace wabt {

namespace {

class ImportResolver: public ExprVisitor::DelegateNop {
 public:
  ImportResolver(unordered_map<string, string>* import_map);

  Result VisitModule(Module* module);

  // Implementation of ExprVisitor::DelegateNop.
  Result BeginBlockExpr(BlockExpr*) override;
  Result EndBlockExpr(BlockExpr*) override;
  Result OnCallExpr(CallExpr*) override;
  Result OnRefFuncExpr(RefFuncExpr*) override;
  Result OnCallIndirectExpr(CallIndirectExpr*) override;
  Result OnReturnCallExpr(ReturnCallExpr*) override;
  Result OnReturnCallIndirectExpr(ReturnCallIndirectExpr*) override;
  Result OnGlobalGetExpr(GlobalGetExpr*) override;
  Result OnGlobalSetExpr(GlobalSetExpr*) override;
  Result BeginIfExpr(IfExpr*) override;
  Result EndIfExpr(IfExpr*) override;
  Result OnLoadExpr(LoadExpr*) override;
  Result BeginLoopExpr(LoopExpr*) override;
  Result EndLoopExpr(LoopExpr*) override;
  Result OnMemoryCopyExpr(MemoryCopyExpr*) override;
  Result OnMemoryFillExpr(MemoryFillExpr*) override;
  Result OnMemoryGrowExpr(MemoryGrowExpr*) override;
  Result OnMemoryInitExpr(MemoryInitExpr*) override;
  Result OnMemorySizeExpr(MemorySizeExpr*) override;
  Result OnTableCopyExpr(TableCopyExpr*) override;
  Result OnTableInitExpr(TableInitExpr*) override;
  Result OnTableGetExpr(TableGetExpr*) override;
  Result OnTableSetExpr(TableSetExpr*) override;
  Result OnTableGrowExpr(TableGrowExpr*) override;
  Result OnTableSizeExpr(TableSizeExpr*) override;
  Result OnTableFillExpr(TableFillExpr*) override;
  Result OnStoreExpr(StoreExpr*) override;
  Result BeginTryExpr(TryExpr*) override;
  Result EndTryExpr(TryExpr*) override;
  Result OnCatchExpr(TryExpr*, Catch*) override;
  Result OnThrowExpr(ThrowExpr*) override;

 private:
  void PushLabel(const std::string& label);
  void PopLabel();
  void ResolveImportForVar(string_view name, Var* var);
  Result ResolveImportForFuncVar(Var* var);
  Result ResolveImportForGlobalVar(Var* var);
  Result ResolveImportForTableVar(Var* var);
  Result ResolveImportForMemoryVar(Var* var);
  Result ResolveImportForTagVar(Var* var);
  Result VisitFunc(Index func_index, Func* func);
  Result VisitGlobal(Global* global);
  Result VisitTag(Tag* tag);
  Result VisitExport(Index export_index, Export* export_);
  Result VisitElemSegment(Index elem_segment_index, ElemSegment* segment);
  Result VisitDataSegment(Index data_segment_index, DataSegment* segment);
  Result VisitStart(Var* start_var);

  Module* module_ = nullptr;
  Func* current_func_ = nullptr;
  unordered_map<string, string>* import_map_;
  ExprVisitor visitor_;
  std::vector<std::string> labels_;
};

ImportResolver::ImportResolver(unordered_map<string, string>* import_map) : import_map_(import_map),
                                                                            visitor_(this) {}

void ImportResolver::PushLabel(const std::string& label) {
  labels_.push_back(label);
}

void ImportResolver::PopLabel() {
  labels_.pop_back();
}

void ImportResolver::ResolveImportForVar(string_view name, Var* var) {
  if (var->is_name()) {
    assert(name == var->name());
  }

  if (!name.empty()) { 
    if (import_map_->find({name.begin(), name.end()}) != import_map_->end()) {
      var->set_name(import_map_->at({name.begin(), name.end()}));
    } else {
      var->set_name(name);
    }
  }

  return;
}

Result ImportResolver::ResolveImportForFuncVar(Var* var) {
  Func* func = module_->GetFunc(*var);
  if (!func) {
    return Result::Error;
  }
  ResolveImportForVar(func->name, var);
  return Result::Ok;
}

Result ImportResolver::ResolveImportForGlobalVar(Var* var) {
  Global* global = module_->GetGlobal(*var);
  if (!global) {
    return Result::Error;
  }
  ResolveImportForVar(global->name, var);
  return Result::Ok;
}

Result ImportResolver::ResolveImportForTableVar(Var* var) {
  Table* table = module_->GetTable(*var);
  if (!table) {
    return Result::Error;
  }
  ResolveImportForVar(table->name, var);
  return Result::Ok;
}

Result ImportResolver::ResolveImportForMemoryVar(Var* var) {
  Memory* memory = module_->GetMemory(*var);
  if (!memory) {
    return Result::Error;
  }
  ResolveImportForVar(memory->name, var);
  return Result::Ok;
}

Result ImportResolver::ResolveImportForTagVar(Var* var) {
  Tag* tag = module_->GetTag(*var);
  if (!tag) {
    return Result::Error;
  }
  ResolveImportForVar(tag->name, var);
  return Result::Ok;
}

Result ImportResolver::BeginBlockExpr(BlockExpr* expr) {
  PushLabel(expr->block.label);
  return Result::Ok;
}

Result ImportResolver::EndBlockExpr(BlockExpr* expr) {
  PopLabel();
  return Result::Ok;
}

Result ImportResolver::BeginLoopExpr(LoopExpr* expr) {
  PushLabel(expr->block.label);
  return Result::Ok;
}

Result ImportResolver::EndLoopExpr(LoopExpr* expr) {
  PopLabel();
  return Result::Ok;
}

Result ImportResolver::OnMemoryCopyExpr(MemoryCopyExpr* expr) {
  CHECK_RESULT(ResolveImportForMemoryVar(&expr->srcmemidx));
  CHECK_RESULT(ResolveImportForMemoryVar(&expr->destmemidx));
  return Result::Ok;
}

Result ImportResolver::OnMemoryFillExpr(MemoryFillExpr* expr) {
  CHECK_RESULT(ResolveImportForMemoryVar(&expr->memidx));
  return Result::Ok;
}

Result ImportResolver::OnMemoryGrowExpr(MemoryGrowExpr* expr) {
  CHECK_RESULT(ResolveImportForMemoryVar(&expr->memidx));
  return Result::Ok;
}

Result ImportResolver::OnMemoryInitExpr(MemoryInitExpr* expr)  {
  CHECK_RESULT(ResolveImportForMemoryVar(&expr->memidx));
  return Result::Ok;
}

Result ImportResolver::OnMemorySizeExpr(MemorySizeExpr* expr) {
  CHECK_RESULT(ResolveImportForMemoryVar(&expr->memidx));
  return Result::Ok;
}

Result ImportResolver::OnTableCopyExpr(TableCopyExpr* expr) {
  CHECK_RESULT(ResolveImportForTableVar(&expr->dst_table));
  CHECK_RESULT(ResolveImportForTableVar(&expr->src_table));
  return Result::Ok;
}

Result ImportResolver::OnTableInitExpr(TableInitExpr* expr)  {
  CHECK_RESULT(ResolveImportForTableVar(&expr->table_index));
  return Result::Ok;
}

Result ImportResolver::OnTableGetExpr(TableGetExpr* expr)  {
  CHECK_RESULT(ResolveImportForTableVar(&expr->var));
  return Result::Ok;
}

Result ImportResolver::OnTableSetExpr(TableSetExpr* expr)  {
  CHECK_RESULT(ResolveImportForTableVar(&expr->var));
  return Result::Ok;
}

Result ImportResolver::OnTableGrowExpr(TableGrowExpr* expr)  {
  CHECK_RESULT(ResolveImportForTableVar(&expr->var));
  return Result::Ok;
}

Result ImportResolver::OnTableSizeExpr(TableSizeExpr* expr)  {
  CHECK_RESULT(ResolveImportForTableVar(&expr->var));
  return Result::Ok;
}

Result ImportResolver::OnTableFillExpr(TableFillExpr* expr)  {
  CHECK_RESULT(ResolveImportForTableVar(&expr->var));
  return Result::Ok;
}

Result ImportResolver::OnStoreExpr(StoreExpr* expr) {
  CHECK_RESULT(ResolveImportForMemoryVar(&expr->memidx));
  return Result::Ok;
}

Result ImportResolver::BeginTryExpr(TryExpr* expr) {
  PushLabel(expr->block.label);
  return Result::Ok;
}

Result ImportResolver::EndTryExpr(TryExpr*) {
  PopLabel();
  return Result::Ok;
}

Result ImportResolver::OnCatchExpr(TryExpr*, Catch* expr) {
  if (!expr->IsCatchAll()) {
    CHECK_RESULT(ResolveImportForTagVar(&expr->var));
  }
  return Result::Ok;
}

Result ImportResolver::OnThrowExpr(ThrowExpr* expr) {
  CHECK_RESULT(ResolveImportForTagVar(&expr->var));
  return Result::Ok;
}

Result ImportResolver::OnCallExpr(CallExpr* expr) {
  CHECK_RESULT(ResolveImportForFuncVar(&expr->var));
  return Result::Ok;
}

Result ImportResolver::OnRefFuncExpr(RefFuncExpr* expr) {
  CHECK_RESULT(ResolveImportForFuncVar(&expr->var));
  return Result::Ok;
}

Result ImportResolver::OnCallIndirectExpr(CallIndirectExpr* expr) {
  CHECK_RESULT(ResolveImportForTableVar(&expr->table));
  return Result::Ok;
}

Result ImportResolver::OnReturnCallExpr(ReturnCallExpr* expr) {
  CHECK_RESULT(ResolveImportForFuncVar(&expr->var));
  return Result::Ok;
}

Result ImportResolver::OnReturnCallIndirectExpr(ReturnCallIndirectExpr* expr) {
  CHECK_RESULT(ResolveImportForTableVar(&expr->table));
  return Result::Ok;
}

Result ImportResolver::OnGlobalGetExpr(GlobalGetExpr* expr) {
  CHECK_RESULT(ResolveImportForGlobalVar(&expr->var));
  return Result::Ok;
}

Result ImportResolver::BeginIfExpr(IfExpr* expr) {
  PushLabel(expr->true_.label);
  return Result::Ok;
}

Result ImportResolver::EndIfExpr(IfExpr* expr) {
  PopLabel();
  return Result::Ok;
}

Result ImportResolver::OnLoadExpr(LoadExpr* expr) {
  CHECK_RESULT(ResolveImportForMemoryVar(&expr->memidx));
  return Result::Ok;
}

Result ImportResolver::OnGlobalSetExpr(GlobalSetExpr* expr) {
  CHECK_RESULT(ResolveImportForGlobalVar(&expr->var));
  return Result::Ok;
}

Result ImportResolver::VisitFunc(Index func_index, Func* func) {
  current_func_ = func;

  CHECK_RESULT(visitor_.VisitFunc(func));
  current_func_ = nullptr;
  return Result::Ok;
}

Result ImportResolver::VisitGlobal(Global* global) {
  CHECK_RESULT(visitor_.VisitExprList(global->init_expr));
  return Result::Ok;
}

Result ImportResolver::VisitTag(Tag* tag) {
  return Result::Ok;
}

Result ImportResolver::VisitExport(Index export_index, Export* export_) {
  switch (export_->kind) {
    case ExternalKind::Func:
      ResolveImportForFuncVar(&export_->var);
      break;

    case ExternalKind::Table:
      ResolveImportForTableVar(&export_->var);
      break;

    case ExternalKind::Memory:
      ResolveImportForMemoryVar(&export_->var);
      break;

    case ExternalKind::Global:
      ResolveImportForGlobalVar(&export_->var);
      break;

    case ExternalKind::Tag:
      ResolveImportForTagVar(&export_->var);
      break;
  }
  return Result::Ok;
}

Result ImportResolver::VisitElemSegment(Index elem_segment_index,
                                     ElemSegment* segment) {
  CHECK_RESULT(ResolveImportForTableVar(&segment->table_var));
  CHECK_RESULT(visitor_.VisitExprList(segment->offset));
  for (ExprList& elem_expr : segment->elem_exprs) {
    Expr* expr = &elem_expr.front();
    if (expr->type() == ExprType::RefFunc) {
      CHECK_RESULT(ResolveImportForFuncVar(&cast<RefFuncExpr>(expr)->var));
    }
  }
  return Result::Ok;
}

Result ImportResolver::VisitDataSegment(Index data_segment_index,
                                     DataSegment* segment) {
  CHECK_RESULT(ResolveImportForMemoryVar(&segment->memory_var));
  CHECK_RESULT(visitor_.VisitExprList(segment->offset));
  return Result::Ok;
}

Result ImportResolver::VisitStart(Var* start_var) {
  CHECK_RESULT(ResolveImportForFuncVar(start_var));
  return Result::Ok;
}

Result ImportResolver::VisitModule(Module* module) {
  module_ = module;
  for (size_t i = 0; i < module->funcs.size(); ++i)
    CHECK_RESULT(VisitFunc(i, module->funcs[i]));
  for (size_t i = 0; i < module->globals.size(); ++i)
    CHECK_RESULT(VisitGlobal(module->globals[i]));
  for (size_t i = 0; i < module->tags.size(); ++i)
    CHECK_RESULT(VisitTag(module->tags[i]));
  for (size_t i = 0; i < module->exports.size(); ++i)
    CHECK_RESULT(VisitExport(i, module->exports[i]));
  for (size_t i = 0; i < module->elem_segments.size(); ++i)
    CHECK_RESULT(VisitElemSegment(i, module->elem_segments[i]));
  for (size_t i = 0; i < module->data_segments.size(); ++i)
    CHECK_RESULT(VisitDataSegment(i, module->data_segments[i]));
  for (size_t i = 0; i < module->starts.size(); ++i)
    CHECK_RESULT(VisitStart(module->starts[i]));
  module_ = nullptr;
  return Result::Ok;
}

void ImportMapConstructor(Module* module_, Module* libmodule, unordered_map<string, string>* import_map) {
  for (Import* import_ : module_->imports) {
    if (import_->module_name == libmodule->name) {
      string field_name = import_->field_name;
      string name;
      
      // Grab the name of import from import object
      switch (import_->kind()) {
        case ExternalKind::Func: {
          Func& func = cast<FuncImport>(import_)->func;
          name = func.name;
          break;
        }

        case ExternalKind::Table: {
          Table& table = cast<TableImport>(import_)->table;
          name = table.name;
          break;
        }

        case ExternalKind::Memory: {
          Memory& memory = cast<MemoryImport>(import_)->memory;
          name = memory.name;
          break;
        }

        case ExternalKind::Global: {
          Global& global = cast<GlobalImport>(import_)->global;
          name = global.name;
          break;
        }

        case ExternalKind::Tag: {
          Tag& tag = cast<TagImport>(import_)->tag;
          name = tag.name;
          break;
        }
      }

      // Grab the name of the item being exported
      Index export_index = libmodule->export_bindings.FindIndex(field_name); 
      Export* export_ = libmodule->exports[export_index];
      string export_name;
      
      switch (export_->kind) {
        case ExternalKind::Func:
          if (Func* func = libmodule->GetFunc(export_->var)) {
            export_name = func->name;
          }
          break;

        case ExternalKind::Table:
          if (Table* table = libmodule->GetTable(export_->var)) {
            export_name = table->name;
          }
          break;

        case ExternalKind::Memory:
          if (Memory* memory = libmodule->GetMemory(export_->var)) {
            export_name = memory->name;
          }
          break;

        case ExternalKind::Global:
          if (Global* global = libmodule->GetGlobal(export_->var)) {
            export_name = global->name;
          }
          break;

        case ExternalKind::Tag:
          if (Tag* tag = libmodule->GetTag(export_->var)) {
            export_name = tag->name;
          }
          break;
      }

      import_map->insert({name, export_name});
    }
  }
}

}  // end anonymous namespace

Result ResolveImports(Module* module, Module* libmodule, unordered_map<string, string>* import_map) {
  ImportMapConstructor(module, libmodule, import_map);
  ImportResolver resolver(import_map);
  return resolver.VisitModule(module);
}

}  // namespace wabt
