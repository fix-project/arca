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

#include <cstdio>
#include <memory>
#include <string>
#include <unordered_set>
#include <vector>

#include "combine-modules.h"
#include "generate-prefix-names.h"
#include "resolve-imports.h"
#include "wabt/apply-names.h"
#include "wabt/binary-reader-ir.h"
#include "wabt/binary-writer.h"
#include "wabt/cast.h"
#include "wabt/error-formatter.h"
#include "wabt/resolve-names.h"
#include "wabt/stream.h"
#include "wabt/validator.h"

using namespace wabt;

static Result Merge(const std::vector<std::string> &inputs,
                    const std::string &destination, bool rename) {
  Features features;
  features.EnableAll();
  Errors errors;
  std::vector<std::unique_ptr<Module>> modules;
  std::vector<Module *> pointers;
  std::unordered_set<std::string> names;
  for (size_t i = 0; i < inputs.size(); i += 2) {
    if (!names.insert(inputs[i + 1]).second) {
      fprintf(stderr, "Duplicate module name: %s\n", inputs[i + 1].c_str());
      return Result::Error;
    }
    std::vector<uint8_t> bytes;
    CHECK_RESULT(ReadFile(inputs[i].c_str(), &bytes));
    auto module = std::make_unique<Module>();
    ReadBinaryOptions options(features, nullptr, false, true, true);
    Result status = ReadBinaryIr(inputs[i].c_str(), bytes.data(), bytes.size(),
                                 options, &errors, module.get());
    if (Succeeded(status)) {
      status = ValidateModule(module.get(), &errors, ValidateOptions(features));
    }
    if (Failed(status)) {
      FormatErrorsToFile(errors, Location::Type::Binary);
      return status;
    }
    module->name = inputs[i + 1];
    CHECK_RESULT(GeneratePrefixNames(module.get()));
    CHECK_RESULT(ApplyNames(module.get()));
    pointers.push_back(module.get());
    modules.push_back(std::move(module));
  }
  CHECK_RESULT(ResolveImports(pointers));

  Module output;
  CHECK_RESULT(CombineModules(pointers, &output, rename));
  Result status = ResolveNamesModule(&output, &errors);
  if (Succeeded(status)) {
    status = ValidateModule(&output, &errors, ValidateOptions(features));
  }
  if (Failed(status)) {
    FormatErrorsToFile(errors, Location::Type::Binary);
    return status;
  }
  WriteBinaryOptions options;
  options.features = features;
  options.features.disable_compact_imports();
  MemoryStream stream;
  CHECK_RESULT(WriteBinaryModule(&stream, &output, options));
  return stream.WriteToFile(destination);
}

int main(int argc, char **argv) {
  InitStdio();
  std::vector<std::string> inputs;
  std::string output;
  bool rename = false;
  for (int i = 1; i < argc; ++i) {
    const std::string argument = argv[i];
    if (argument == "-o" && i + 1 < argc) {
      output = argv[++i];
    } else if (argument == "--rename-export-conflicts") {
      rename = true;
    } else if (argument == "--all-features" || argument == "--disable-gc") {
      continue;
    } else if (!argument.empty() && argument[0] == '-') {
      fprintf(stderr, "Unknown option: %s\n", argument.c_str());
      return 1;
    } else {
      inputs.push_back(argument);
    }
  }
  if (output.empty() || inputs.empty() || inputs.size() % 2 != 0) {
    fprintf(stderr, "Usage: wasm-merge INPUT NAME ... -o OUTPUT "
                    "[--rename-export-conflicts]\n");
    return 1;
  }
  WABT_TRY
  return Failed(Merge(inputs, output, rename));
  WABT_CATCH_BAD_ALLOC_AND_EXIT
}
