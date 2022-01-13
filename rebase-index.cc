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

#include "rebase-index.h"

#include <cassert>
#include <cstdio>

#include "src/cast.h"
#include "src/expr-visitor.h"
#include "src/ir.h"
#include "src/wast-lexer.h"

namespace wabt {

namespace {

class MemoryIndexRebaser : public ExprVisitor::DelegateNop {
 public:
  MemoryIndexRebaser(Script* script, Errors* errors, Index memidx_base);

  Result VisitModule(Module* module);
  Result VisitScript(Script* script);

  // Implementation of ExprVisitor::DelegateNop.
  Result OnLoadExpr(LoadExpr*) override;
  Result OnMemoryCopyExpr(MemoryCopyExpr*) override;
  Result OnMemoryFillExpr(MemoryFillExpr*) override;
  Result OnMemoryGrowExpr(MemoryGrowExpr*) override;
  Result OnMemoryInitExpr(MemoryInitExpr*) override;
  Result OnMemorySizeExpr(MemorySizeExpr*) override;
  Result OnStoreExpr(StoreExpr*) override;

 private:
  void VisitFunc(Func* func);
  void VisitExport(Export* export_);
  void VisitGlobal(Global* global);
  void VisitTag(Tag* tag);
  void VisitElemSegment(ElemSegment* segment);
  void VisitDataSegment(DataSegment* segment);
  void VisitScriptModule(ScriptModule* script_module);
  void VisitCommand(Command* command);
  void RebaseMemoryIndex(Var* memidx);

  Errors* errors_ = nullptr;
  Script* script_ = nullptr;
  Module* current_module_ = nullptr;
  Func* current_func_ = nullptr;
  Index memidx_base_;
  ExprVisitor visitor_;
  std::vector<std::string> labels_;
  Result result_ = Result::Ok;
};

MemoryIndexRebaser::MemoryIndexRebaser(Script* script, Errors* errors, Index memidx_base)
    : errors_(errors),
      script_(script),
      memidx_base_(memidx_base),
      visitor_(this) {}

Result MemoryIndexRebaser::OnLoadExpr(LoadExpr* expr) {
  RebaseMemoryIndex(&expr->memidx);
  return Result::Ok;
}

Result MemoryIndexRebaser::OnMemoryCopyExpr(MemoryCopyExpr* expr) {
  RebaseMemoryIndex(&expr->srcmemidx);
  RebaseMemoryIndex(&expr->destmemidx);
  return Result::Ok;
}

Result MemoryIndexRebaser::OnMemoryFillExpr(MemoryFillExpr* expr) {
  RebaseMemoryIndex(&expr->memidx);
  return Result::Ok;
}

Result MemoryIndexRebaser::OnMemoryGrowExpr(MemoryGrowExpr* expr) {
  RebaseMemoryIndex(&expr->memidx);
  return Result::Ok;
}

Result MemoryIndexRebaser::OnMemoryInitExpr(MemoryInitExpr* expr) {
  RebaseMemoryIndex(&expr->memidx);
  return Result::Ok;
}

Result MemoryIndexRebaser::OnMemorySizeExpr(MemorySizeExpr* expr) {
  RebaseMemoryIndex(&expr->memidx);
  return Result::Ok;
}

Result MemoryIndexRebaser::OnStoreExpr(StoreExpr* expr) {
  RebaseMemoryIndex(&expr->memidx);
  return Result::Ok;
}

void MemoryIndexRebaser::VisitFunc(Func* func) {
  current_func_ = func;
  visitor_.VisitFunc(func);
  current_func_ = nullptr;
}

void MemoryIndexRebaser::VisitExport(Export* export_) {
  return;
}

void MemoryIndexRebaser::VisitGlobal(Global* global) {
  visitor_.VisitExprList(global->init_expr);
}

void MemoryIndexRebaser::VisitTag(Tag* tag) {
  return;
}

void MemoryIndexRebaser::VisitElemSegment(ElemSegment* segment) {
  visitor_.VisitExprList(segment->offset);
}

void MemoryIndexRebaser::VisitDataSegment(DataSegment* segment) {
  visitor_.VisitExprList(segment->offset);
}

Result MemoryIndexRebaser::VisitModule(Module* module) {
  current_module_ = module;

  for (Func* func : module->funcs)
    VisitFunc(func);
  for (Export* export_ : module->exports)
    VisitExport(export_);
  for (Global* global : module->globals)
    VisitGlobal(global);
  for (Tag* tag : module->tags)
    VisitTag(tag);
  for (ElemSegment* elem_segment : module->elem_segments)
    VisitElemSegment(elem_segment);
  for (DataSegment* data_segment : module->data_segments)
    VisitDataSegment(data_segment);
  current_module_ = nullptr;
  return result_;
}

void MemoryIndexRebaser::RebaseMemoryIndex(Var* memidx) {
  if (current_module_->GetMemoryIndex(*memidx) >= current_module_->num_memory_imports) {
    memidx->set_index(memidx->index() + memidx_base_ - 1);
  }
  return;
}

void MemoryIndexRebaser::VisitScriptModule(ScriptModule* script_module) {
  if (auto* tsm = dyn_cast<TextScriptModule>(script_module)) {
    VisitModule(&tsm->module);
  }
}

}  // end anonymous namespace

Result RebaseIndexModule(Module* module, Errors* errors, Index memidx_base) {
  MemoryIndexRebaser rebaser(nullptr, errors, memidx_base);
  return rebaser.VisitModule(module);
}

}  // namespace wabt
